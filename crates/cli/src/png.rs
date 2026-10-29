//! Writing a PNG, with no dependency to pull in.
//!
//! A capture is a debugging artefact: it has to open in anything, it has to be byte-identical for the
//! same picture, and it must not add a decoder stack to a crate that only needs to write. So the
//! encoder is here: an 8-bit RGBA image, filter type zero on every row, and zlib's stored blocks -
//! which is why there is no compression level to argue about.

/// The eight bytes every PNG starts with.
const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// The CRC table, built once per call because a capture is not on any hot path.
fn crc_table() -> [u32; 256] {
    let mut table = [0_u32; 256];
    for (index, slot) in table.iter_mut().enumerate() {
        let mut value = u32::try_from(index).unwrap_or(0);
        for _ in 0..8 {
            value = if value & 1 == 1 {
                0xedb8_8320 ^ (value >> 1)
            } else {
                value >> 1
            };
        }
        *slot = value;
    }
    table
}

fn crc32(bytes: &[u8]) -> u32 {
    let table = crc_table();
    let mut crc = 0xffff_ffff_u32;
    for byte in bytes {
        let index = usize::try_from((crc ^ u32::from(*byte)) & 0xff).unwrap_or(0);
        crc = table.get(index).copied().unwrap_or(0) ^ (crc >> 8);
    }
    crc ^ 0xffff_ffff
}

fn adler32(bytes: &[u8]) -> u32 {
    let mut low = 1_u32;
    let mut high = 0_u32;
    for byte in bytes {
        low = (low + u32::from(*byte)) % 65521;
        high = (high + low) % 65521;
    }
    (high << 16) | low
}

/// One PNG chunk: length, name, payload, checksum.
fn chunk(kind: [u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let length = u32::try_from(payload.len()).unwrap_or(0);
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(&kind);
    out.extend_from_slice(payload);
    let mut checked = Vec::with_capacity(kind.len() + payload.len());
    checked.extend_from_slice(&kind);
    checked.extend_from_slice(payload);
    out.extend_from_slice(&crc32(&checked).to_be_bytes());
    out
}

/// Wraps bytes in zlib's frame, in stored blocks: uncompressed, and therefore exactly reproducible.
fn zlib(bytes: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    if bytes.is_empty() {
        out.extend_from_slice(&[1, 0x00, 0x00, 0xff, 0xff]);
    } else {
        for (index, block) in bytes.chunks(65_535).enumerate() {
            let last = (index + 1) * 65_535 >= bytes.len();
            out.push(u8::from(last));
            let length = u16::try_from(block.len()).unwrap_or(0);
            out.extend_from_slice(&length.to_le_bytes());
            out.extend_from_slice(&(!length).to_le_bytes());
            out.extend_from_slice(block);
        }
    }
    out.extend_from_slice(&adler32(bytes).to_be_bytes());
    out
}

/// A picture, in the only colour model a capture needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// How many pixels across.
    pub width: u32,
    /// How many pixels down.
    pub height: u32,
    /// Four bytes per pixel, row by row, from the top left.
    pub rgba: Vec<u8>,
}

impl Image {
    /// A picture of one colour.
    #[must_use]
    pub fn blank(width: u32, height: u32, colour: [u8; 4]) -> Self {
        let mut rgba = Vec::new();
        for _ in 0..width * height {
            rgba.extend_from_slice(&colour);
        }
        Self {
            width,
            height,
            rgba,
        }
    }

    /// Paints one pixel, doing nothing for a pixel outside the picture.
    pub fn set(&mut self, x: u32, y: u32, colour: [u8; 4]) {
        if x >= self.width || y >= self.height {
            return;
        }
        let index = usize::try_from((y * self.width + x) * 4).unwrap_or(0);
        if let Some(slot) = self.rgba.get_mut(index..index + 4) {
            slot.copy_from_slice(&colour);
        }
    }

    /// Fills a rectangle, clipped to the picture.
    pub fn fill(&mut self, x: u32, y: u32, width: u32, height: u32, colour: [u8; 4]) {
        for row in y..y.saturating_add(height) {
            for column in x..x.saturating_add(width) {
                self.set(column, row, colour);
            }
        }
    }

    /// The file, as bytes.
    #[must_use]
    pub fn png(&self) -> Vec<u8> {
        let mut scanlines = Vec::new();
        for row in 0..self.height {
            scanlines.push(0);
            let start = usize::try_from(row * self.width * 4).unwrap_or(0);
            let end = start + usize::try_from(self.width * 4).unwrap_or(0);
            scanlines.extend_from_slice(self.rgba.get(start..end).unwrap_or(&[]));
        }
        let mut header = Vec::new();
        header.extend_from_slice(&self.width.to_be_bytes());
        header.extend_from_slice(&self.height.to_be_bytes());
        header.extend_from_slice(&[8, 6, 0, 0, 0]);

        let mut out = SIGNATURE.to_vec();
        out.extend_from_slice(&chunk(*b"IHDR", &header));
        out.extend_from_slice(&chunk(*b"IDAT", &zlib(&scanlines)));
        out.extend_from_slice(&chunk(*b"IEND", &[]));
        out
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::expect_used)]
    use super::*;

    /// The payload of one chunk of a file.
    fn chunk_of(file: &[u8], wanted: [u8; 4]) -> Vec<u8> {
        let mut at = 8;
        loop {
            let length = usize::try_from(u32::from_be_bytes([
                file.get(at).copied().expect("length"),
                file.get(at + 1).copied().expect("length"),
                file.get(at + 2).copied().expect("length"),
                file.get(at + 3).copied().expect("length"),
            ]))
            .expect("a length");
            let kind = file.get(at + 4..at + 8).expect("a chunk name");
            let body = file.get(at + 8..at + 8 + length).expect("a chunk body").to_vec();
            if kind == wanted.as_slice() {
                return body;
            }
            at += 12 + length;
        }
    }

    /// Reads a stored-block zlib stream back, which is the only kind this writer makes.
    fn unstored(bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut at = 2;
        loop {
            let last = bytes.get(at).copied().expect("a block header") & 1 == 1;
            let length = usize::from(
                u16::from_le_bytes([
                    bytes.get(at + 1).copied().expect("length"),
                    bytes.get(at + 2).copied().expect("length"),
                ]),
            );
            out.extend_from_slice(bytes.get(at + 5..at + 5 + length).expect("block body"));
            at += 5 + length;
            if last {
                break;
            }
        }
        out
    }

    #[test]
    fn a_picture_comes_back_out_of_the_file_it_went_into() {
        let mut image = Image::blank(3, 2, [0, 0, 0, 255]);
        image.set(1, 0, [10, 20, 30, 255]);
        image.fill(0, 1, 2, 1, [40, 50, 60, 255]);

        let file = image.png();

        assert_eq!(file.get(..8), Some(&SIGNATURE[..]), "the signature is a PNG's");
        let scanlines = unstored(&chunk_of(&file, *b"IDAT"));
        // Rows are one filter byte plus the pixels, and the filter byte of every row is zero.
        assert_eq!(scanlines.len(), 2 * (1 + 3 * 4));
        assert_eq!(scanlines.get(1 + 4..1 + 8), Some(&[10, 20, 30, 255][..]), "the pixel set on the first row is where it was put");
        assert_eq!(
            scanlines.get(13 + 1..13 + 9),
            Some(&[40, 50, 60, 255, 40, 50, 60, 255][..]),
            "and the filled row is filled"
        );
    }

    #[test]
    fn the_same_picture_is_the_same_file_twice() {
        let image = Image::blank(4, 4, [1, 2, 3, 255]);
        assert_eq!(image.png(), image.png(), "a capture can be compared byte for byte");
    }
}
