//! How fast a character reads.
//!
//! Reference: `Character::read_speed()` (`character.cpp:3669-3684`), which turns intelligence into a duration per "unit" of reading, clamps it, and answers **a percentage of a minute** - the units
//! every reading activity multiplies its time by.

/// The percentage of a minute a character with this intelligence takes to read one unit.
///
/// Reference: `Character::read_speed()`:
///
/// ```text
/// const float intel = 2.0f + get_int() / 8.0f;
/// time_duration ret = 180_seconds / intel;
/// ret = enchantment_cache->modify_value( READING_SPEED_MULTIPLIER, ret );   // P5, not ported
/// if( ret < 1_seconds ) { ret = 1_seconds; }
/// return ret * 100 / 1_minutes;
/// ```
///
/// **The comment in the reference gives the whole curve and is the gate's oracle:** *4 int = 72 seconds, 8 int = 60 seconds, 12 int = 51 seconds, 16 int = 45 seconds, 20 int = 40 seconds* - nine
/// percent a point at the bottom, under four at the top. The duration is a whole number of seconds, so 51.4 becomes **51**, and the answer is `seconds * 100 / 60`.
#[must_use]
pub fn read_speed_percent(intelligence: i32) -> i32 {
    let intel = 2.0 + f64::from(intelligence) / 8.0;
    // The duration truncates to whole seconds, as `time_duration` does when 180 is divided by a float.
    #[allow(clippy::cast_possible_truncation)]
    let mut seconds = (180.0 / intel).trunc() as i32;
    if seconds < 1 {
        seconds = 1;
    }
    seconds * 100 / 60
}

/// How long a book takes to read, before anything about the book's identity is known.
///
/// Reference: `Character::time_to_read` (`character.cpp:12411-12436`), the three numeric steps that need no book data:
///
/// ```text
/// time_duration retval = type->time * reading_speed / 100;
/// retval *= std::min( fine_detail_vision_mod(), reader.fine_detail_vision_mod() );
/// if( type->intel > effective_int ) retval += type->time * ( from_seconds( type->intel - effective_int ) / 1_minutes );
/// ```
///
/// **The intelligence penalty is counted in whole minutes**, because the shortfall in seconds is divided by `1_minutes` before it multiplies - so **a shortfall of 30 seconds adds nothing**, and 90
/// seconds adds the same as 60. That is the integer division the gate exists for, and it is the sort of rule a hand-written "...plus a bit per point" would get wrong in the generous direction.
///
/// The caller supplies the **smaller** of the two vision modifiers, which is the reference's `std::min`: the worse eyes win.
#[must_use]
pub fn time_to_read(base_seconds: i64, speed_percent: i32, vision_modifier: i32, intel_shortfall: i32) -> i64 {
    let mut seconds = base_seconds * i64::from(speed_percent) / 100;
    seconds *= i64::from(vision_modifier);
    if intel_shortfall > 0 {
        // Whole minutes of shortfall, in the reference's own order: the duration is divided before it multiplies.
        seconds += base_seconds * i64::from(intel_shortfall / 60);
    }
    seconds
}

/// What skimming a book the character has not identified costs: a tenth of the time.
///
/// Reference: `if( !has_identified( book.typeId() ) ) { retval /= 10; }` (`character.cpp:12433-12435`).
#[must_use]
pub fn skimmed(seconds: i64) -> i64 {
    seconds / 10
}

/// A book's own reading time, put through every scaling its reader brings.
///
/// Reference: `Character::time_to_read` (`character.cpp:12411-12436`) - the speed, the worse of the two vision modifiers, the whole-minute intelligence shortfall, **and then** `retval /= 10` for a
/// book the character has not identified.
///
/// **The order is the rule, and it is the end that surprises:** skimming tenths a time that has **already been stretched**, so a slow reader in the dark skims a longer ten minutes than a quick one;
/// doing the division first and the scaling after would compress every difference the reader brings into a tenth of itself.
#[must_use]
pub fn book_time_seconds(
    base_seconds: i64,
    speed_percent: i32,
    vision_modifier: i32,
    intel_shortfall: i32,
    identified: bool,
) -> i64 {
    let full = time_to_read(base_seconds, speed_percent, vision_modifier, intel_shortfall);
    if identified { full } else { skimmed(full) }
}

/// The parts of a book that decide how much fun it is, and of a reader that change the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunBook {
    /// Its stated fun, which may be negative (`islot_book::fun`).
    pub fun: i32,
    /// How many chapters it has (`item::get_chapters`).
    pub chapters: i32,
    /// How many of them this reader has left (`item::get_remaining_chapters`).
    pub remaining: i32,
}

/// How much fun a particular reader gets from a particular book.
///
/// Reference: `Character::book_fun_for` (`character.cpp:11958-11992`), whose rules run **in this order**:
///
/// ```text
/// int fun_bonus = book.type->book->fun;
/// if( ( CANNIBAL || PSYCHOPATH || SAPIOVORE ) && book == cookbook_human ) fun_bonus = abs( fun_bonus );
/// else if( SPIRITUAL && book.has_flag( INSPIRATIONAL ) )            fun_bonus = abs( fun_bonus * 3 );
/// if( LOVES_BOOKS ) fun_bonus++;
/// else if( HATES_BOOKS ) { if( book.fun > 0 ) fun_bonus = 0; else fun_bonus--; }
/// if( fun_bonus > 1 && book.get_chapters() > 0 && book.get_remaining_chapters( p ) == 0 ) fun_bonus /= 2;
/// ```
///
/// **Four things about that order are easy to lose.** The first two are an `if / else if`, so a cannibal spiritualist gets the cannibal rule alone. `HATES_BOOKS` **zeroes** a good book instead of
/// flipping it, and takes one more off a book that was already no fun - so hate makes a bad book worse and a good book merely dull. The chapter halving comes **last**, so it halves what the traits did
/// rather than what the book stated. And the halving needs `fun_bonus > 1` **after** the traits, so a book of fun one that loves-books makes two is halved.
#[must_use]
pub fn book_fun_for(book: FunBook, traits: &[&str], inspirational: bool, human_cookbook: bool) -> i32 {
    let has = |trait_name: &str| traits.contains(&trait_name);
    let mut fun = book.fun;
    if (has("CANNIBAL") || has("PSYCHOPATH") || has("SAPIOVORE")) && human_cookbook {
        fun = fun.abs();
    } else if has("SPIRITUAL") && inspirational {
        fun = (fun * 3).abs();
    }
    if has("LOVES_BOOKS") {
        fun += 1;
    } else if has("HATES_BOOKS") {
        if book.fun > 0 {
            fun = 0;
        } else {
            fun -= 1;
        }
    }
    if fun > 1 && book.chapters > 0 && book.remaining == 0 {
        fun /= 2;
    }
    fun
}

/// Whether a reader would call the book fun at all - the reference's `fun_to_read` (`character.cpp:11953-11956`), which is `book_fun_for > 0`.
#[must_use]
pub fn fun_to_read(book: FunBook, traits: &[&str], inspirational: bool, human_cookbook: bool) -> bool {
    book_fun_for(book, traits, inspirational, human_cookbook) > 0
}

/// Whether a reader is allowed to read this book at all.
///
/// Reference: `avatar::read`'s condition table (`character.cpp:12267-12271`):
///
/// ```text
/// if( !fun_to_read( book ) && !has_morale_to_read() && has_identified( book.typeId() ) ) result |= MORALE_LOW;
/// ```
///
/// **All three conditions must hold for the refusal**, and the third is the half a reader notices: an **unidentified** book can always be read, however miserable the character, because there is still
/// something to learn from it. A book that is still fun needs no cheering up either.
#[must_use]
pub const fn morale_permits_reading(fun_to_read: bool, morale_level: i32, identified: bool) -> bool {
    fun_to_read || crate::morale::has_morale_to_read(morale_level) || !identified
}

/// What a reader could get out of a book.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookMastery {
    /// The book has not been identified, so nobody can say what is in it.
    CantDetermine,
    /// It teaches nothing the reader can use - either it teaches no skill at all, or they already know everything in it.
    Mastered,
    /// The reader is below the level it requires.
    CantUnderstand,
    /// There is something to learn.
    Learning,
}

/// Which of the four verdicts a book gets from a reader.
///
/// Reference: `Character::get_book_mastery` (`character.cpp:11925-11951`):
///
/// ```text
/// if( !book.is_book() || !has_identified( book.typeId() ) ) return CANT_DETERMINE;
/// if( !skill ) return MASTERED;                     // the book gives no skill
/// if( skill_requirement > skill_level ) return CANT_UNDERSTAND;
/// if( skill_level >= max_skill_learnable ) return MASTERED;
/// return LEARNING;
/// ```
///
/// **Two boundaries carry the whole rule.** `req > knowledge` is the understanding gate, so **exactly the required level understands** - a `>=` there would lock out every reader who meets the
/// requirement. And `knowledge >= level` is the mastery gate, so **exactly the level the book teaches is already mastered**, not one short.
#[must_use]
pub fn book_mastery(
    identified: bool,
    has_skill: bool,
    knowledge_level: i32,
    skill_requirement: i32,
    max_skill_learnable: i32,
) -> BookMastery {
    if !identified {
        return BookMastery::CantDetermine;
    }
    if !has_skill {
        return BookMastery::Mastered;
    }
    if skill_requirement > knowledge_level {
        return BookMastery::CantUnderstand;
    }
    if knowledge_level >= max_skill_learnable {
        return BookMastery::Mastered;
    }
    BookMastery::Learning
}
