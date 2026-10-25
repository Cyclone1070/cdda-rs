//! Physical quantities: mass, volume, length, energy, power, angle and money.
//!
//! Reference: `src/units.h` (the `quantity` template and every conversion) and
//! `tests/units_test.cpp` (the reference's own expectations, 202 assertions).
//!
//! A quantity stores one number in its base unit — milligrams for mass,
//! millilitres for volume, millimetres for length, millijoules for energy,
//! milliwatts for power and cents for money — exactly as the reference does.
//! C++ overloads the user-defined literals on the literal's type; Rust has no
//! such feature, so same-named macros dispatch through [`Literal`].

use core::cmp::Ordering;
use core::f64::consts::PI;
use core::fmt;
use core::marker::PhantomData;
use core::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

/// A physical quantity: a value in its base unit, tagged with the dimension it
/// measures.
///
/// Reference: `quantity` (`src/units.h:28-154`). The tag keeps quantities of
/// different dimensions from being added or compared by mistake.
pub struct Quantity<V, U> {
    value: V,
    unit: PhantomData<U>,
}

impl<V, U> Quantity<V, U> {
    /// Wraps a value that is already expressed in the base unit.
    ///
    /// Prefer a named conversion such as [`from_gram`]: the tag alone does not
    /// say which unit the number is in.
    #[must_use]
    pub const fn new(value: V) -> Self {
        Self {
            value,
            unit: PhantomData,
        }
    }
}

impl<V: Copy, U> Quantity<V, U> {
    /// The raw base-unit value (`src/units.h:70`).
    #[must_use]
    pub const fn value(self) -> V {
        self.value
    }
}

// The unit tag is phantom data, so a quantity is always copyable no matter what
// the tag is; deriving would wrongly require the tag to be `Copy` itself.
impl<V: Copy, U> Copy for Quantity<V, U> {}

impl<V: Copy, U> Clone for Quantity<V, U> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V: PartialEq, U> PartialEq for Quantity<V, U> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<V: PartialOrd, U> PartialOrd for Quantity<V, U> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.value.partial_cmp(&other.value)
    }
}

impl<V: Value + Neg<Output = V>, U> Neg for Quantity<V, U> {
    type Output = Self;

    fn neg(self) -> Self {
        Self::new(-self.value)
    }
}

// Addition and subtraction of one dimension, `src/units.h:98-143`.
impl<V: Value + Add<Output = V>, U> Add for Quantity<V, U> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self::new(self.value + rhs.value)
    }
}

impl<V: Value + Sub<Output = V>, U> Sub for Quantity<V, U> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self::new(self.value - rhs.value)
    }
}

impl<V: Value + Add<Output = V>, U> AddAssign for Quantity<V, U> {
    fn add_assign(&mut self, rhs: Self) {
        self.value = self.value + rhs.value;
    }
}

impl<V: Value + Sub<Output = V>, U> SubAssign for Quantity<V, U> {
    fn sub_assign(&mut self, rhs: Self) {
        self.value = self.value - rhs.value;
    }
}

// `quantity * scalar` and `quantity / scalar`, `src/units.h:218-223` and
// `:240-245`; the result stays in the quantity's own type.
impl<V: Value + Mul<Output = V>, U> Mul<V> for Quantity<V, U> {
    type Output = Self;

    fn mul(self, factor: V) -> Self {
        Self::new(self.value * factor)
    }
}

impl<V: Value + Div<Output = V>, U> Div<V> for Quantity<V, U> {
    type Output = Self;

    fn div(self, divisor: V) -> Self {
        Self::new(self.value / divisor)
    }
}

// `scalar * quantity` (`src/units.h:209-215`) for the scalar types the port
// uses so far.
impl<U> Mul<Quantity<f64, U>> for f64 {
    type Output = Quantity<f64, U>;

    fn mul(self, rhs: Quantity<f64, U>) -> Self::Output {
        Quantity::new(self * rhs.value)
    }
}

impl<U> Mul<Quantity<i64, U>> for i64 {
    type Output = Quantity<i64, U>;

    fn mul(self, rhs: Quantity<i64, U>) -> Self::Output {
        Quantity::new(self * rhs.value)
    }
}

impl<V: fmt::Debug, U: Unit> fmt::Debug for Quantity<V, U> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}{}", self.value, U::SYMBOL)
    }
}

/// Short symbol of a base unit, printed by [`Quantity`]'s `Debug` output.
///
/// Reference: the streaming operators at `src/units.h:751-795`.
pub trait Unit {
    /// The symbol, e.g. `"mg"`.
    const SYMBOL: &'static str;
}

/// Numeric value a [`Quantity`] can store.
///
/// The reference instantiates `quantity` with `int`, `int64_t`, `float` and
/// `double` (`src/units_fwd.h`). The integral and `double` cases are covered
/// here, which is what the conversion helpers need; `float` quantities such as
/// [`Temperature`] are converted by their own functions because adding them to
/// a delta changes the result type.
pub trait Value: Copy + PartialOrd {
    /// Largest value that `scale(1000)` does not saturate, matching
    /// `std::numeric_limits<value_type>::max() / 1000` (`src/units.h:511`).
    const MAX_SCALED: Self;

    /// Multiplies by `factor`, saturating instead of overflowing.
    ///
    /// The reference multiplies raw integers and relies on callers to stay in
    /// range (`src/units.h:514`); for every in-range input the result is the
    /// same.
    #[must_use]
    fn scale(self, factor: i32) -> Self;

    /// Widens to `f64`.
    ///
    /// Integers above 2^53 lose precision, just as the reference's
    /// `value_type / 1000.0` does.
    fn to_f64(self) -> f64;

    /// Narrows from `f64` by truncating toward zero, matching the reference's
    /// implicit `double` to integer conversion.
    fn from_f64(value: f64) -> Self;
}

impl Value for i32 {
    const MAX_SCALED: Self = Self::MAX / 1000;

    fn scale(self, factor: i32) -> Self {
        self.saturating_mul(factor)
    }

    fn to_f64(self) -> f64 {
        f64::from(self)
    }

    #[allow(clippy::cast_possible_truncation)]
    fn from_f64(value: f64) -> Self {
        value as Self
    }
}

impl Value for i64 {
    const MAX_SCALED: Self = Self::MAX / 1000;

    fn scale(self, factor: i32) -> Self {
        self.saturating_mul(i64::from(factor))
    }

    #[allow(clippy::cast_precision_loss)]
    fn to_f64(self) -> f64 {
        self as f64
    }

    #[allow(clippy::cast_possible_truncation)]
    fn from_f64(value: f64) -> Self {
        value as Self
    }
}

impl Value for f64 {
    const MAX_SCALED: Self = Self::MAX / 1000.0;

    fn scale(self, factor: i32) -> Self {
        self * f64::from(factor)
    }

    fn to_f64(self) -> f64 {
        self
    }

    fn from_f64(value: f64) -> Self {
        value
    }
}

/// Millilitres: the base unit of [`Volume`] (`src/units_fwd.h`).
pub struct VolumeInMilliliter;

impl Unit for VolumeInMilliliter {
    const SYMBOL: &'static str = "ml";
}

/// Volume in `int` millilitres.
///
/// `using volume = quantity<int, volume_in_milliliter_tag>`
/// (`src/units_fwd.h`).
pub type Volume = Quantity<i32, VolumeInMilliliter>;

/// Wraps millilitres unchanged (`src/units.h:306-311`).
#[must_use]
pub const fn from_milliliter<V: Value>(v: V) -> Quantity<V, VolumeInMilliliter> {
    Quantity::new(v)
}

/// Litres in millilitres (`src/units.h:313-317`).
#[must_use]
pub fn from_liter<V: Value>(v: V) -> Quantity<V, VolumeInMilliliter> {
    from_milliliter(v.scale(1000))
}

/// Millilitres unchanged (`src/units.h:319-323`).
#[must_use]
pub fn to_milliliter<V: Value>(v: Quantity<V, VolumeInMilliliter>) -> V {
    v.value()
}

/// Litres as a `double`, keeping the fraction (`src/units.h:325-328`).
#[must_use]
pub fn to_liter<V: Value>(v: Quantity<V, VolumeInMilliliter>) -> f64 {
    v.value().to_f64() / 1000.0
}

/// Milligrams: the base unit of [`Mass`] (`src/units_fwd.h`).
pub struct MassInMilligram;

impl Unit for MassInMilligram {
    const SYMBOL: &'static str = "mg";
}

/// Mass in `int64_t` milligrams.
///
/// `using mass = quantity<std::int64_t, mass_in_milligram_tag>`
/// (`src/units_fwd.h`).
pub type Mass = Quantity<i64, MassInMilligram>;

/// Wraps milligrams unchanged (`src/units.h:330-335`).
#[must_use]
pub const fn from_milligram<V: Value>(v: V) -> Quantity<V, MassInMilligram> {
    Quantity::new(v)
}

/// Grams in milligrams (`src/units.h:337-342`).
#[must_use]
pub fn from_gram<V: Value>(v: V) -> Quantity<V, MassInMilligram> {
    from_milligram(v.scale(1000))
}

/// Kilograms in milligrams (`src/units.h:344-349`).
#[must_use]
pub fn from_kilogram<V: Value>(v: V) -> Quantity<V, MassInMilligram> {
    from_gram(v.scale(1000))
}

/// Milligrams unchanged (`src/units.h:351-355`).
#[must_use]
pub fn to_milligram<V: Value>(v: Quantity<V, MassInMilligram>) -> V {
    v.value()
}

/// Whole grams, rounded toward zero (`src/units.h:357-361`).
#[must_use]
pub fn to_gram<V: Value>(v: Quantity<V, MassInMilligram>) -> V {
    V::from_f64(v.value().to_f64() / 1000.0)
}

/// Kilograms as a `double` (`src/units.h:363-366`).
#[must_use]
pub fn to_kilogram<V: Value>(v: Quantity<V, MassInMilligram>) -> f64 {
    v.value().to_f64() / 1_000_000.0
}

/// Joules per gram: the base unit of [`SpecificEnergy`] (`src/units_fwd.h`).
pub struct SpecificEnergyInJoulePerGram;

impl Unit for SpecificEnergyInJoulePerGram {
    const SYMBOL: &'static str = "J/g";
}

/// Specific energy in `float` joules per gram.
///
/// `using specific_energy = quantity<float, specific_energy_in_joule_per_gram_tag>`
/// (`src/units_fwd.h`).
pub type SpecificEnergy = Quantity<f32, SpecificEnergyInJoulePerGram>;

/// Wraps joules per gram unchanged (`src/units.h:368-374`).
#[must_use]
pub const fn from_joule_per_gram<V: Value>(v: V) -> Quantity<V, SpecificEnergyInJoulePerGram> {
    Quantity::new(v)
}

/// Joules per gram unchanged (`src/units.h:376-381`).
#[must_use]
pub fn to_joule_per_gram<V: Value>(v: Quantity<V, SpecificEnergyInJoulePerGram>) -> V {
    v.value()
}

/// Kelvins: the base unit of [`Temperature`] (`src/units_fwd.h`).
pub struct TemperatureInKelvin;

impl Unit for TemperatureInKelvin {
    const SYMBOL: &'static str = "K";
}

/// Absolute temperature in `float` kelvins.
///
/// `using temperature = quantity<float, temperature_in_kelvin_tag>`
/// (`src/units_fwd.h`).
pub type Temperature = Quantity<f32, TemperatureInKelvin>;

/// Wraps kelvins unchanged (`src/units.h:383-388`).
#[must_use]
pub const fn from_kelvin(v: f32) -> Temperature {
    Quantity::new(v)
}

/// Kelvins unchanged (`src/units.h:390-395`).
#[must_use]
pub const fn to_kelvin(v: Temperature) -> f32 {
    v.value()
}

/// Celsius in kelvins (`src/units.h:397-402`); the reference adds `273.150f`.
#[must_use]
pub fn from_celsius(v: f32) -> Temperature {
    from_kelvin(v + 273.150)
}

/// Fahrenheit in kelvins (`src/units.h:404-410`).
#[must_use]
pub fn from_fahrenheit(v: f32) -> Temperature {
    from_kelvin((v + 459.67) / 1.8)
}

/// Celsius from kelvins (`src/units.h:412-417`).
#[must_use]
pub fn to_celsius(v: Temperature) -> f32 {
    v.value() - 273.150
}

/// Fahrenheit from kelvins (`src/units.h:419-424`).
#[must_use]
pub fn to_fahrenheit(v: Temperature) -> f32 {
    v.value() * 1.8 - 459.67
}

/// Kelvin deltas: the base unit of [`TemperatureDelta`] (`src/units_fwd.h`).
pub struct TemperatureDeltaInKelvin;

impl Unit for TemperatureDeltaInKelvin {
    const SYMBOL: &'static str = "K";
}

/// A temperature difference in `float` kelvins.
///
/// `using temperature_delta = quantity<float, temperature_delta_in_kelvin_tag>`
/// (`src/units_fwd.h`).
pub type TemperatureDelta = Quantity<f32, TemperatureDeltaInKelvin>;

/// Wraps kelvins unchanged (`src/units.h:441-447`).
#[must_use]
pub const fn from_kelvin_delta(v: f32) -> TemperatureDelta {
    Quantity::new(v)
}

/// Kelvin deltas unchanged (`src/units.h:449-454`).
#[must_use]
pub const fn to_kelvin_delta(v: TemperatureDelta) -> f32 {
    v.value()
}

/// Celsius deltas are kelvin deltas (`src/units.h:456-462`).
#[must_use]
pub const fn from_celsius_delta(v: f32) -> TemperatureDelta {
    from_kelvin_delta(v)
}

/// Fahrenheit deltas are `1.8` smaller (`src/units.h:464-470`).
#[must_use]
pub fn from_fahrenheit_delta(v: f32) -> TemperatureDelta {
    from_kelvin_delta(v / 1.8)
}

/// Celsius deltas unchanged (`src/units.h:472-477`).
#[must_use]
pub const fn to_celsius_delta(v: TemperatureDelta) -> f32 {
    v.value()
}

/// Fahrenheit deltas are `1.8` larger (`src/units.h:479-484`).
#[must_use]
pub fn to_fahrenheit_delta(v: TemperatureDelta) -> f32 {
    v.value() * 1.8
}

// `temperature - temperature` yields a delta (`src/units.cpp:222-225`). It is
// written out for `f32` because the result changes type, which the generic
// same-type operator cannot express.
impl Sub<Temperature> for Temperature {
    type Output = TemperatureDelta;

    fn sub(self, rhs: Temperature) -> TemperatureDelta {
        TemperatureDelta::new(self.value - rhs.value)
    }
}

// `temperature + delta` and `delta + temperature` stay absolute
// (`src/units.cpp:227-240`), and `+=` follows (`src/units.cpp:242-246`).
impl Add<TemperatureDelta> for Temperature {
    type Output = Temperature;

    fn add(self, rhs: TemperatureDelta) -> Temperature {
        Temperature::new(self.value + rhs.value)
    }
}

impl Add<Temperature> for TemperatureDelta {
    type Output = Temperature;

    fn add(self, rhs: Temperature) -> Temperature {
        Temperature::new(self.value + rhs.value)
    }
}

impl AddAssign<TemperatureDelta> for Temperature {
    fn add_assign(&mut self, rhs: TemperatureDelta) {
        self.value += rhs.value;
    }
}

/// Millijoules: the base unit of [`Energy`] (`src/units_fwd.h`).
pub struct EnergyInMillijoule;

impl Unit for EnergyInMillijoule {
    const SYMBOL: &'static str = "mJ";
}

/// Energy in `int64_t` millijoules.
///
/// `using energy = quantity<std::int64_t, energy_in_millijoule_tag>`
/// (`src/units_fwd.h`).
pub type Energy = Quantity<i64, EnergyInMillijoule>;

/// Wraps millijoules unchanged (`src/units.h:500-507`).
#[must_use]
pub const fn from_millijoule<V: Value>(v: V) -> Quantity<V, EnergyInMillijoule> {
    Quantity::new(v)
}

/// Joules in millijoules, clamped before scaling so the product cannot
/// overflow (`src/units.h:508-516`).
#[must_use]
pub fn from_joule<V: Value>(v: V) -> Quantity<V, EnergyInMillijoule> {
    let energy = if v > V::MAX_SCALED { V::MAX_SCALED } else { v };
    from_millijoule(energy.scale(1000))
}

/// Kilojoules in millijoules, clamped the same way (`src/units.h:518-527`).
#[must_use]
pub fn from_kilojoule<V: Value>(v: V) -> Quantity<V, EnergyInMillijoule> {
    let joules = v.scale(1000);
    let energy = if joules > V::MAX_SCALED {
        V::MAX_SCALED
    } else {
        joules
    };
    from_joule(energy)
}

/// Millijoules unchanged (`src/units.h:529-533`).
#[must_use]
pub fn to_millijoule<V: Value>(v: Quantity<V, EnergyInMillijoule>) -> V {
    v.value()
}

/// Whole joules, rounded toward zero (`src/units.h:535-539`).
#[must_use]
pub fn to_joule<V: Value>(v: Quantity<V, EnergyInMillijoule>) -> V {
    V::from_f64(v.value().to_f64() / 1000.0)
}

/// Whole kilojoules, rounded toward zero (`src/units.h:541-545`).
#[must_use]
pub fn to_kilojoule<V: Value>(v: Quantity<V, EnergyInMillijoule>) -> V {
    V::from_f64(to_joule(v).to_f64() / 1000.0)
}

/// Energy for display, in the largest unit that divides it exactly.
///
/// Reference: `units::display( const units::energy & )` (`src/units.cpp`,
/// declared at `src/units.h:805`). Each step is tested with the *already
/// truncated* whole number, so `1_000_001` mJ still reads as `1 kJ`.
#[must_use]
// The reference compares the double division against exactly 1000
// (`src/units.cpp`); the comparison is the whole point of the function.
#[allow(clippy::float_cmp)]
pub fn display_energy(v: Energy) -> String {
    let kj = to_kilojoule(v);
    let j = to_joule(v);
    // At least 1 kJ and no fraction.
    if kj >= 1 && j.to_f64() / kj.to_f64() == 1000.0 {
        return format!("{kj} kJ");
    }
    let mj = to_millijoule(v);
    // At least 1 J and no fraction.
    if j >= 1 && mj.to_f64() / j.to_f64() == 1000.0 {
        return format!("{j} J");
    }
    format!("{mj} mJ")
}

/// Milliwatts: the base unit of [`Power`] (`src/units_fwd.h`).
pub struct PowerInMilliwatt;

impl Unit for PowerInMilliwatt {
    const SYMBOL: &'static str = "mW";
}

/// Power in `int64_t` milliwatts.
///
/// `using power = quantity<std::int64_t, power_in_milliwatt_tag>`
/// (`src/units_fwd.h`).
pub type Power = Quantity<i64, PowerInMilliwatt>;

/// Wraps milliwatts unchanged (`src/units.h:547-553`).
#[must_use]
pub const fn from_milliwatt<V: Value>(v: V) -> Quantity<V, PowerInMilliwatt> {
    Quantity::new(v)
}

/// Watts in milliwatts, clamped before scaling (`src/units.h:555-563`).
#[must_use]
pub fn from_watt<V: Value>(v: V) -> Quantity<V, PowerInMilliwatt> {
    let power = if v > V::MAX_SCALED { V::MAX_SCALED } else { v };
    from_milliwatt(power.scale(1000))
}

/// Kilowatts in milliwatts, clamped the same way (`src/units.h:565-574`).
#[must_use]
pub fn from_kilowatt<V: Value>(v: V) -> Quantity<V, PowerInMilliwatt> {
    let watts = v.scale(1000);
    let power = if watts > V::MAX_SCALED {
        V::MAX_SCALED
    } else {
        watts
    };
    from_watt(power)
}

/// Milliwatts unchanged (`src/units.h:576-580`).
#[must_use]
pub fn to_milliwatt<V: Value>(v: Quantity<V, PowerInMilliwatt>) -> V {
    v.value()
}

/// Whole watts, rounded toward zero (`src/units.h:582-586`).
#[must_use]
pub fn to_watt<V: Value>(v: Quantity<V, PowerInMilliwatt>) -> V {
    V::from_f64(v.value().to_f64() / 1000.0)
}

/// Whole kilowatts, rounded toward zero (`src/units.h:588-592`).
#[must_use]
pub fn to_kilowatt<V: Value>(v: Quantity<V, PowerInMilliwatt>) -> V {
    V::from_f64(to_watt(v).to_f64() / 1000.0)
}

/// Cents: the base unit of [`Money`] (`src/units_fwd.h`).
pub struct MoneyInCent;

impl Unit for MoneyInCent {
    const SYMBOL: &'static str = "cent";
}

/// Money in `int` cents.
///
/// `using money = quantity<int, money_in_cent_tag>` (`src/units_fwd.h`).
pub type Money = Quantity<i32, MoneyInCent>;

/// Wraps cents unchanged (`src/units.h:594-600`).
#[must_use]
pub const fn from_cent<V: Value>(v: V) -> Quantity<V, MoneyInCent> {
    Quantity::new(v)
}

/// Dollars in cents (`src/units.h:602-606`).
#[must_use]
pub fn from_usd<V: Value>(v: V) -> Quantity<V, MoneyInCent> {
    from_cent(v.scale(100))
}

/// Kilodollars in cents (`src/units.h:608-612`).
#[must_use]
pub fn from_kusd<V: Value>(v: V) -> Quantity<V, MoneyInCent> {
    from_usd(v.scale(1000))
}

/// Cents unchanged (`src/units.h:614-618`).
#[must_use]
pub fn to_cent<V: Value>(v: Quantity<V, MoneyInCent>) -> V {
    v.value()
}

/// Whole dollars, rounded toward zero (`src/units.h:620-624`).
#[must_use]
pub fn to_usd<V: Value>(v: Quantity<V, MoneyInCent>) -> V {
    V::from_f64(v.value().to_f64() / 100.0)
}

/// Whole kilodollars, rounded toward zero (`src/units.h:626-630`).
#[must_use]
pub fn to_kusd<V: Value>(v: Quantity<V, MoneyInCent>) -> V {
    V::from_f64(to_usd(v).to_f64() / 1000.0)
}

/// Millimetres: the base unit of [`Length`] (`src/units_fwd.h`).
pub struct LengthInMillimeter;

impl Unit for LengthInMillimeter {
    const SYMBOL: &'static str = "mm";
}

/// Length in `int` millimetres.
///
/// `using length = quantity<int, length_in_millimeter_tag>`
/// (`src/units_fwd.h`).
pub type Length = Quantity<i32, LengthInMillimeter>;

/// Wraps millimetres unchanged (`src/units.h:632-637`).
#[must_use]
pub const fn from_millimeter<V: Value>(v: V) -> Quantity<V, LengthInMillimeter> {
    Quantity::new(v)
}

/// Centimetres in millimetres (`src/units.h:639-644`).
#[must_use]
pub fn from_centimeter<V: Value>(v: V) -> Quantity<V, LengthInMillimeter> {
    from_millimeter(v.scale(10))
}

/// Metres in millimetres (`src/units.h:646-651`).
#[must_use]
pub fn from_meter<V: Value>(v: V) -> Quantity<V, LengthInMillimeter> {
    from_millimeter(v.scale(1000))
}

/// Kilometres in millimetres (`src/units.h:653-658`).
#[must_use]
pub fn from_kilometer<V: Value>(v: V) -> Quantity<V, LengthInMillimeter> {
    from_millimeter(v.scale(1_000_000))
}

/// Millimetres unchanged (`src/units.h:660-664`).
#[must_use]
pub fn to_millimeter<V: Value>(v: Quantity<V, LengthInMillimeter>) -> V {
    v.value()
}

/// Whole centimetres, rounded toward zero (`src/units.h:666-670`).
#[must_use]
pub fn to_centimeter<V: Value>(v: Quantity<V, LengthInMillimeter>) -> V {
    V::from_f64(v.value().to_f64() / 10.0)
}

/// Whole metres, rounded toward zero (`src/units.h:672-676`).
#[must_use]
pub fn to_meter<V: Value>(v: Quantity<V, LengthInMillimeter>) -> V {
    V::from_f64(v.value().to_f64() / 1000.0)
}

/// Whole kilometres, rounded toward zero (`src/units.h:678-682`).
#[must_use]
pub fn to_kilometer<V: Value>(v: Quantity<V, LengthInMillimeter>) -> V {
    V::from_f64(v.value().to_f64() / 1_000_000.0)
}

/// Radians: the base unit of [`Angle`] (`src/units_fwd.h`).
pub struct AngleInRadians;

impl Unit for AngleInRadians {
    const SYMBOL: &'static str = "rad";
}

/// Angle in `double` radians.
///
/// `using angle = quantity<double, angle_in_radians_tag>` (`src/units_fwd.h`).
pub type Angle = Quantity<f64, AngleInRadians>;

/// Wraps radians unchanged (`src/units.h:684-688`).
#[must_use]
pub const fn from_radians(v: f64) -> Angle {
    Quantity::new(v)
}

/// Degrees in radians (`src/units.h:695-699`).
#[must_use]
pub fn from_degrees<V: Value>(v: V) -> Angle {
    from_radians(v.to_f64() * PI / 180.0)
}

/// Arcminutes in radians (`src/units.h:706-710`).
#[must_use]
pub fn from_arcmin<V: Value>(v: V) -> Angle {
    from_degrees(v.to_f64() / 60.0)
}

/// Sine of an angle (`src/units.h:1083-1086`).
#[must_use]
pub fn sin(a: Angle) -> f64 {
    a.value().sin()
}

/// Cosine of an angle (`src/units.h:1088-1091`).
#[must_use]
pub fn cos(a: Angle) -> f64 {
    a.value().cos()
}

/// Angle whose tangent is `y / x`, like `std::atan2` (`src/units.h:1103-1106`).
#[must_use]
pub fn atan2(y: f64, x: f64) -> Angle {
    from_radians(y.atan2(x))
}

/// Rounds to the nearest multiple of `of`, ties away from zero.
///
/// Reference: `round_to_multiple_of` (`src/units_utility.h:40-45`), which uses
/// `std::lround` and then scales `of` by the resulting whole number.
#[must_use]
pub fn round_to_multiple_of<V: Value, U>(
    val: Quantity<V, U>,
    of: Quantity<V, U>,
) -> Quantity<V, U> {
    let multiple = (val.value().to_f64() / of.value().to_f64()).round();
    Quantity::new(V::from_f64(multiple * of.value().to_f64()))
}

/// Dispatch for the literal macros.
///
/// C++ overloads each user-defined literal on the literal's type: an integer
/// literal yields the canonical quantity type from `src/units_fwd.h`, while a
/// floating literal yields `quantity<double, _>` (`src/units.h:836-866`). The
/// macros call this trait instead; the integer fallback to `i32` selects the
/// canonical type.
pub trait Literal {
    /// Quantity an integer literal produces for the volume dimension.
    type Volume;
    /// Quantity an integer literal produces for the mass dimension.
    type Mass;
    /// Quantity an integer literal produces for the energy dimension.
    type Energy;
    /// Quantity an integer literal produces for the power dimension.
    type Power;
    /// Quantity an integer literal produces for the money dimension.
    type Money;
    /// Quantity an integer literal produces for the length dimension.
    type Length;
    /// Quantity an integer literal produces for the angle dimension.
    type Angle;

    /// `1_milliliter` (`src/units.h:812-821`).
    #[must_use]
    fn milliliter(self) -> Self::Volume;
    /// `1_liter` (`src/units.h:824-833`).
    #[must_use]
    fn liter(self) -> Self::Volume;

    /// `1_milligram` (`src/units.h:836-839`).
    #[must_use]
    fn milligram(self) -> Self::Mass;
    /// `1_gram` (`src/units.h:840-843`).
    #[must_use]
    fn gram(self) -> Self::Mass;
    /// `1_kilogram` (`src/units.h:845-848`).
    #[must_use]
    fn kilogram(self) -> Self::Mass;

    /// `1_millijoule` (`src/units.h:899-908`).
    #[must_use]
    fn millijoule(self) -> Self::Energy;
    /// `1_joule` (`src/units.h:910-919`).
    #[must_use]
    fn joule(self) -> Self::Energy;
    /// `1_kilojoule` (`src/units.h:921-930`).
    #[must_use]
    fn kilojoule(self) -> Self::Energy;

    /// `1_milliwatt` (`src/units.h:932-935`).
    #[must_use]
    fn milliwatt(self) -> Self::Power;
    /// `1_watt` (`src/units.h:937-940`).
    #[must_use]
    fn watt(self) -> Self::Power;
    /// `1_kilowatt` (`src/units.h:942-945`).
    #[must_use]
    fn kilowatt(self) -> Self::Power;

    /// `1_cent` (`src/units.h:947-956`).
    #[must_use]
    fn cent(self) -> Self::Money;
    /// `1_USD` (`src/units.h:958-967`).
    #[must_use]
    fn usd(self) -> Self::Money;
    /// `1_kUSD` (`src/units.h:969-978`).
    #[must_use]
    fn kusd(self) -> Self::Money;

    /// `1_millimeter` (`src/units.h:980-989`).
    #[must_use]
    fn millimeter(self) -> Self::Length;
    /// `1_centimeter` (`src/units.h:991-1000`).
    #[must_use]
    fn centimeter(self) -> Self::Length;
    /// `1_meter` (`src/units.h:1002-1011`).
    #[must_use]
    fn meter(self) -> Self::Length;
    /// `1_kilometer` (`src/units.h:1013-1022`).
    #[must_use]
    fn kilometer(self) -> Self::Length;

    /// `1_radians` (`src/units.h:1024-1032`).
    #[must_use]
    fn radians(self) -> Self::Angle;
    /// `1_pi_radians` (`src/units.h:1034-1042`).
    #[must_use]
    fn pi_radians(self) -> Self::Angle;
    /// `1_degrees` (`src/units.h:1044-1052`).
    #[must_use]
    fn degrees(self) -> Self::Angle;
    /// `1_arcmin` (`src/units.h:1054-1062`).
    #[must_use]
    fn arcmin(self) -> Self::Angle;
}

impl Literal for i32 {
    type Volume = Volume;
    type Mass = Mass;
    type Energy = Energy;
    type Power = Power;
    type Money = Money;
    type Length = Length;
    type Angle = Angle;

    fn milliliter(self) -> Volume {
        from_milliliter(self)
    }

    fn liter(self) -> Volume {
        from_liter(self)
    }

    fn milligram(self) -> Mass {
        from_milligram(i64::from(self))
    }

    fn gram(self) -> Mass {
        from_gram(i64::from(self))
    }

    fn kilogram(self) -> Mass {
        from_kilogram(i64::from(self))
    }

    fn millijoule(self) -> Energy {
        from_millijoule(i64::from(self))
    }

    fn joule(self) -> Energy {
        from_joule(i64::from(self))
    }

    fn kilojoule(self) -> Energy {
        from_kilojoule(i64::from(self))
    }

    fn milliwatt(self) -> Power {
        from_milliwatt(i64::from(self))
    }

    fn watt(self) -> Power {
        from_watt(i64::from(self))
    }

    fn kilowatt(self) -> Power {
        from_kilowatt(i64::from(self))
    }

    fn cent(self) -> Money {
        from_cent(self)
    }

    fn usd(self) -> Money {
        from_usd(self)
    }

    fn kusd(self) -> Money {
        from_kusd(self)
    }

    fn millimeter(self) -> Length {
        from_millimeter(self)
    }

    fn centimeter(self) -> Length {
        from_centimeter(self)
    }

    fn meter(self) -> Length {
        from_meter(self)
    }

    fn kilometer(self) -> Length {
        from_kilometer(self)
    }

    fn radians(self) -> Angle {
        from_radians(f64::from(self))
    }

    fn pi_radians(self) -> Angle {
        from_radians(f64::from(self) * PI)
    }

    fn degrees(self) -> Angle {
        from_degrees(self)
    }

    fn arcmin(self) -> Angle {
        from_arcmin(self)
    }
}

impl Literal for f64 {
    type Volume = Quantity<f64, VolumeInMilliliter>;
    type Mass = Quantity<f64, MassInMilligram>;
    type Energy = Quantity<f64, EnergyInMillijoule>;
    type Power = Quantity<f64, PowerInMilliwatt>;
    type Money = Quantity<f64, MoneyInCent>;
    type Length = Quantity<f64, LengthInMillimeter>;
    type Angle = Angle;

    fn milliliter(self) -> Self::Volume {
        from_milliliter(self)
    }

    fn liter(self) -> Self::Volume {
        from_liter(self)
    }

    fn milligram(self) -> Self::Mass {
        from_milligram(self)
    }

    fn gram(self) -> Self::Mass {
        from_gram(self)
    }

    fn kilogram(self) -> Self::Mass {
        from_kilogram(self)
    }

    fn millijoule(self) -> Self::Energy {
        from_millijoule(self)
    }

    fn joule(self) -> Self::Energy {
        from_joule(self)
    }

    fn kilojoule(self) -> Self::Energy {
        from_kilojoule(self)
    }

    fn milliwatt(self) -> Self::Power {
        from_milliwatt(self)
    }

    fn watt(self) -> Self::Power {
        from_watt(self)
    }

    fn kilowatt(self) -> Self::Power {
        from_kilowatt(self)
    }

    fn cent(self) -> Self::Money {
        from_cent(self)
    }

    fn usd(self) -> Self::Money {
        from_usd(self)
    }

    fn kusd(self) -> Self::Money {
        from_kusd(self)
    }

    fn millimeter(self) -> Self::Length {
        from_millimeter(self)
    }

    fn centimeter(self) -> Self::Length {
        from_centimeter(self)
    }

    fn meter(self) -> Self::Length {
        from_meter(self)
    }

    fn kilometer(self) -> Self::Length {
        from_kilometer(self)
    }

    fn radians(self) -> Angle {
        from_radians(self)
    }

    fn pi_radians(self) -> Angle {
        from_radians(self * PI)
    }

    fn degrees(self) -> Angle {
        from_degrees(self)
    }

    fn arcmin(self) -> Angle {
        from_arcmin(self)
    }
}

/// `1_milliliter` (`src/units.h:812`).
#[macro_export]
macro_rules! milliliter {
    ($value:expr) => {
        $crate::units::Literal::milliliter($value)
    };
}

/// `1_liter` (`src/units.h:824`).
#[macro_export]
macro_rules! liter {
    ($value:expr) => {
        $crate::units::Literal::liter($value)
    };
}

/// `1_millijoule` (`src/units.h:899`).
#[macro_export]
macro_rules! millijoule {
    ($value:expr) => {
        $crate::units::Literal::millijoule($value)
    };
}

/// `1_joule` (`src/units.h:910`).
#[macro_export]
macro_rules! joule {
    ($value:expr) => {
        $crate::units::Literal::joule($value)
    };
}

/// `1_kilojoule` (`src/units.h:921`).
#[macro_export]
macro_rules! kilojoule {
    ($value:expr) => {
        $crate::units::Literal::kilojoule($value)
    };
}

/// `1_milliwatt` (`src/units.h:932`).
#[macro_export]
macro_rules! milliwatt {
    ($value:expr) => {
        $crate::units::Literal::milliwatt($value)
    };
}

/// `1_watt` (`src/units.h:937`).
#[macro_export]
macro_rules! watt {
    ($value:expr) => {
        $crate::units::Literal::watt($value)
    };
}

/// `1_kilowatt` (`src/units.h:942`).
#[macro_export]
macro_rules! kilowatt {
    ($value:expr) => {
        $crate::units::Literal::kilowatt($value)
    };
}

/// `1_cent` (`src/units.h:947`).
#[macro_export]
macro_rules! cent {
    ($value:expr) => {
        $crate::units::Literal::cent($value)
    };
}

/// `1_USD` (`src/units.h:958`).
#[macro_export]
macro_rules! usd {
    ($value:expr) => {
        $crate::units::Literal::usd($value)
    };
}

/// `1_kUSD` (`src/units.h:969`).
#[macro_export]
macro_rules! kusd {
    ($value:expr) => {
        $crate::units::Literal::kusd($value)
    };
}

/// `1_millimeter` (`src/units.h:980`).
#[macro_export]
macro_rules! millimeter {
    ($value:expr) => {
        $crate::units::Literal::millimeter($value)
    };
}

/// `1_centimeter` (`src/units.h:991`).
#[macro_export]
macro_rules! centimeter {
    ($value:expr) => {
        $crate::units::Literal::centimeter($value)
    };
}

/// `1_meter` (`src/units.h:1002`).
#[macro_export]
macro_rules! meter {
    ($value:expr) => {
        $crate::units::Literal::meter($value)
    };
}

/// `1_kilometer` (`src/units.h:1013`).
#[macro_export]
macro_rules! kilometer {
    ($value:expr) => {
        $crate::units::Literal::kilometer($value)
    };
}

/// `1_radians` (`src/units.h:1024`).
#[macro_export]
macro_rules! radians {
    ($value:expr) => {
        $crate::units::Literal::radians($value)
    };
}

/// `1_pi_radians` (`src/units.h:1034`).
#[macro_export]
macro_rules! pi_radians {
    ($value:expr) => {
        $crate::units::Literal::pi_radians($value)
    };
}

/// `1_degrees` (`src/units.h:1044`).
#[macro_export]
macro_rules! degrees {
    ($value:expr) => {
        $crate::units::Literal::degrees($value)
    };
}

/// `1_arcmin` (`src/units.h:1054`).
#[macro_export]
macro_rules! arcmin {
    ($value:expr) => {
        $crate::units::Literal::arcmin($value)
    };
}

/// `1_milligram` (`src/units.h:836`).
#[macro_export]
macro_rules! milligram {
    ($value:expr) => {
        $crate::units::Literal::milligram($value)
    };
}

/// `1_gram` (`src/units.h:840`).
#[macro_export]
macro_rules! gram {
    ($value:expr) => {
        $crate::units::Literal::gram($value)
    };
}

/// `1_kilogram` (`src/units.h:845`).
#[macro_export]
macro_rules! kilogram {
    ($value:expr) => {
        $crate::units::Literal::kilogram($value)
    };
}

/// Why a quantity written as text could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnitError {
    /// The string was empty or all spaces.
    Empty,
    /// A term ended without a unit.
    MissingUnit,
    /// A term named a unit the table does not have.
    UnknownUnit,
    /// A term did not start with a number.
    NumberExpected,
}

impl std::fmt::Display for UnitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnitError::Empty => f.write_str("invalid quantity string: empty string"),
            UnitError::MissingUnit => f.write_str("invalid quantity string: missing unit"),
            UnitError::UnknownUnit => f.write_str("invalid quantity string: unknown unit"),
            UnitError::NumberExpected => f.write_str("invalid quantity string: number expected"),
        }
    }
}

impl std::error::Error for UnitError {}

/// Reads a quantity: whole numbers, each followed by a unit, summed.
///
/// Reference: `read_from_json_string_common` (`units.h:1207-1265`). There is no
/// decimal point in the grammar because every unit table is integer-valued, so
/// `"1.5 L"` fails at the `.` rather than rounding.
///
/// # Errors
///
/// Reports which part of the string did not parse.
pub fn read_quantity(text: &str, units: &[(&str, i64)]) -> Result<i64, UnitError> {
    let bytes = text.as_bytes();
    let mut at = 0usize;
    let skip_spaces = |at: &mut usize| -> bool {
        while bytes.get(*at) == Some(&b' ') {
            *at += 1;
        }
        *at >= bytes.len()
    };
    if skip_spaces(&mut at) {
        return Err(UnitError::Empty);
    }
    let mut total = 0i64;
    loop {
        let mut sign = 1i64;
        match bytes.get(at) {
            Some(b'-') => {
                sign = -1;
                at += 1;
            }
            Some(b'+') => at += 1,
            _ => {}
        }
        let mut value = 0i64;
        let mut digits = 0usize;
        while let Some(byte) = bytes.get(at).filter(|byte| byte.is_ascii_digit()) {
            value = value * 10 + i64::from(byte - b'0');
            at += 1;
            digits += 1;
        }
        if digits == 0 {
            return Err(UnitError::NumberExpected);
        }
        if skip_spaces(&mut at) {
            return Err(UnitError::MissingUnit);
        }
        let mut found = None;
        for (unit, scale) in units {
            if text.get(at..).is_some_and(|rest| rest.starts_with(unit)) {
                at += unit.len();
                found = Some(*scale);
                break;
            }
        }
        let Some(scale) = found else {
            return Err(UnitError::UnknownUnit);
        };
        total += sign * value * scale;
        if skip_spaces(&mut at) {
            return Ok(total);
        }
    }
}

/// The mass table (`units.h:1130-1134`), in milligrams.
const MASS_UNITS: &[(&str, i64)] = &[("mg", 1), ("g", 1_000), ("kg", 1_000_000)];

/// The volume table (`units.h:1152-1155`), in millilitres.
const VOLUME_UNITS: &[(&str, i64)] = &[("ml", 1), ("L", 1_000)];

/// The length table (`units.h:1153-1160`), in millimetres.
const LENGTH_UNITS: &[(&str, i64)] = &[("mm", 1), ("cm", 10), ("meter", 1_000), ("km", 1_000_000)];

/// The money table (`units.h:1136-1143`), in cents.
const MONEY_UNITS: &[(&str, i64)] = &[
    ("cents", 1),
    ("cent", 1),
    ("dollars", 100),
    ("dollar", 100),
    ("USD", 100),
    ("kUSD", 100_000),
];

/// Reads a mass in milligrams.
///
/// # Errors
///
/// See [`read_quantity`].
pub fn read_mass(text: &str) -> Result<i64, UnitError> {
    read_quantity(text, MASS_UNITS)
}

/// The vitamin mass table (`src/stomach.h:28-35`), in **micrograms**.
///
/// Vitamins are their own unit tag: `vitamin_units::mass` counts micrograms and knows only these five spellings, which is why a vitamin's amount cannot be read with
/// the item mass reader - it has no micrograms at all, and `calcium`'s `10 mg 420 μg` would fail on the second term.
const VITAMIN_MASS_UNITS: &[(&str, i64)] = &[("ug", 1), ("μg", 1), ("mcg", 1), ("mg", 1_000), ("g", 1_000_000)];

/// Reads a vitamin mass in micrograms.
///
/// # Errors
///
/// See [`read_quantity`].
pub fn read_vitamin_mass(text: &str) -> Result<i64, UnitError> {
    read_quantity(text, VITAMIN_MASS_UNITS)
}

/// Reads a volume in millilitres.
///
/// # Errors
///
/// See [`read_quantity`].
pub fn read_volume(text: &str) -> Result<i64, UnitError> {
    read_quantity(text, VOLUME_UNITS)
}

/// Reads a length in millimetres.
///
/// # Errors
///
/// See [`read_quantity`].
pub fn read_length(text: &str) -> Result<i64, UnitError> {
    read_quantity(text, LENGTH_UNITS)
}

/// Reads money in cents.
///
/// # Errors
///
/// See [`read_quantity`].
pub fn read_money(text: &str) -> Result<i64, UnitError> {
    read_quantity(text, MONEY_UNITS)
}
