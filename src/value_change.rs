use std::str::FromStr;
use std::vec::Vec;

use vcd;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
	Scalar(ScalarValue),
	BinaryVector{width: usize, value: Vec<ScalarValue>},
	Real(f64)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScalarValue {
	V0, V1, X, Z
}

impl Into<vcd::Value> for ScalarValue {
	fn into(self) -> vcd::Value {
		match self {
			ScalarValue::V0 => vcd::Value::V0,
			ScalarValue::V1 => vcd::Value::V1,
			ScalarValue::X => vcd::Value::X,
			ScalarValue::Z => vcd::Value::Z
		}
	}
}

impl FromStr for ScalarValue {
	type Err = ();
	fn from_str(s: &str) -> Result<ScalarValue, ()> {
		match s {
			"0" => Ok(ScalarValue::V0),
			"1" => Ok(ScalarValue::V1),
			"x" | "X" => Ok(ScalarValue::X),
			"z" | "Z" => Ok(ScalarValue::Z),
			_ => Err(())
		}
	}
}

/// Log syntax:
///
/// ```
/// #timestamp signal_name value < size | f >
/// ```
///
/// timestamp: integer in the range [0,2^64)
///
/// signal_name: The name of the signal. Must start with an alphabet character (a-zA-Z).
///
/// value: the value, followed by either the size for a scalar/binary vector, or f for a floating point value.
///
#[derive(Clone, Debug, PartialEq)]
pub struct ValueChange {
	pub timestamp: u64,
	pub signal_name: String,
	pub value: Value
}

#[derive(Debug, Clone)]
pub enum ParseValueChangeError {
	InvalidFormat,
	ParseTimestampErr,
	InvalidValueType,
	InvalidValue,
	ValueTooLargeForVecWidth
}

impl FromStr for ValueChange {
	type Err = ParseValueChangeError;

	fn from_str(s: &str) -> Result<ValueChange,ParseValueChangeError> {
		use regex::Regex;

		lazy_static! {
			static ref RE: Regex = Regex::new(r#"#(\d+)\s([a-zA-Z0-9.]+)\s([01xXzZ]+|\d+\.\d+)\s(\d+|f)"#).unwrap();
		}

		let s = s.trim();
		let caps = RE.captures(s).ok_or(ParseValueChangeError::InvalidFormat)?;

		let timestamp_str = caps.get(1).unwrap().as_str();
		let name_str = caps.get(2).unwrap().as_str();
		let value_str = caps.get(3).unwrap().as_str();
		let value_type_str = caps.get(4).unwrap().as_str();

		// try to parse timestamp and value from captured groups
		let timestamp = u64::from_str(timestamp_str).map_err(|_| ParseValueChangeError::ParseTimestampErr)?;
		let value = if value_type_str == "f" {
			let real = f64::from_str(value_str).map_err(|_| ParseValueChangeError::InvalidValue)?;
			Value::Real(real)
		}
		else {
			// try to parse value_type_str as an integer
			match usize::from_str(value_type_str) {
				Ok(1) => {
					let value = ScalarValue::from_str(value_str).map_err(|_| ParseValueChangeError::InvalidValue)?;
					Value::Scalar(value)
				},
				Ok(width) => {
					let mut vec = Vec::with_capacity(s.len());
					for c in value_str.chars() {
						match c {
							'0' => vec.push(ScalarValue::V0),
							'1' => vec.push(ScalarValue::V1),
							'x' | 'X' => vec.push(ScalarValue::X),
							'z' | 'Z' => vec.push(ScalarValue::Z),
							_ => return Err(ParseValueChangeError::InvalidValue)
						};
					}
					if vec.len() > width {
						return Err(ParseValueChangeError::ValueTooLargeForVecWidth);
					}
					Value::BinaryVector{width, value: vec}
				},
				Err(_e) => {
					return Err(ParseValueChangeError::InvalidValueType)
				}
			}
		};

		Ok(ValueChange {
			timestamp,
			signal_name: String::from(name_str),
			value
		})
	}
}


#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn scalar() {
		let s = "#100 imasignal 1 1";
		let result = ValueChange::from_str(s).unwrap();
		let expected = ValueChange {
			timestamp: 100,
			signal_name: String::from("imasignal"),
			value: Value::Scalar(ScalarValue::V1)
		};
		assert_eq!(expected, result);
	}

	#[test]
	fn scalar2() {
		let s = "#1283075 AFC003.Outputs.D1 1 1";
		let result = ValueChange::from_str(s).unwrap();
		let expected = ValueChange {
			timestamp: 1283075,
			signal_name: String::from("AFC003.Outputs.D1"),
			value: Value::Scalar(ScalarValue::V1)
		};
		assert_eq!(expected, result);
	}

	#[test]
	#[should_panic]
	fn invalid_scalar() {
		let s = "#100 imasignal 2 1"; // scalars must be 1 or 0
		let _ = ValueChange::from_str(s).unwrap();
	}

	#[test]
	fn vec8() {
		use super::ScalarValue::*;
		let s = "#100 signame 11110010 8";
		let result = ValueChange::from_str(s).unwrap();
		assert_eq!(Value::BinaryVector{width: 8, value: vec![V1, V1, V1, V1, V0, V0, V1, V0]}, result.value)
	}

	#[test]
	#[should_panic]
	fn vec8_value_too_large() {
		let s = "#100 signame 111111111 8"; //value needs 9-bits
		let _ = ValueChange::from_str(s).unwrap();
	}

	#[test]
	fn real() {
		let s = "#222 signame 123.4 f";
		let result = ValueChange::from_str(s).unwrap();
		assert_eq!(Value::Real(123.4f64), result.value);
	}

	#[test]
	#[should_panic]
	fn value_value_type_mismatch() {
		let s = "#222 signame 123.4 8";
		let _ = ValueChange::from_str(s).unwrap();
	}
}
