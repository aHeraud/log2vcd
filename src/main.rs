// vcd specification:
// http://staff.ustc.edu.cn/~songch/download/IEEE.1364-2005.pdf

#[macro_use]
extern crate structopt;

#[macro_use]
extern crate lazy_static;

extern crate chrono;
extern crate vcd;
extern crate regex;

use std::path::PathBuf;
use std::str::FromStr;
use std::fs::File;
use std::io::{ Read, Write, BufReader, BufRead, stdin, stdout };
use std::collections::HashMap;

use structopt::StructOpt;
use vcd::TimescaleUnit;

mod value_change;

use self::value_change::ValueChange;

#[derive(StructOpt, Debug)]
struct Options {
	#[structopt(short = "i", long = "input_file", parse(from_os_str))]
	/// Log file to read from, if no input file is provided, input will be read from stdin
	input_path: Option<PathBuf>,

	#[structopt(short = "o", long = "output_file", parse(from_os_str))]
	/// The file to write the output to, if no file is provided, the output will be printed to stdout
	output_path: Option<PathBuf>,

	#[structopt(short = "u", long = "unit", parse(try_from_str))]
	/// Timescale unit, must be one of: { 'S', 'MS', 'US', 'NS', 'PS', 'FS' }
	unit: TimescaleUnit,

	#[structopt(long = "step_size", parse(try_from_str), default_value = "1")]
	/// Timescale step size
	step_size: u32
}

fn main() {
	use vcd::{Writer, IdCode, Var, VarType, Header, Scope, ScopeItem, ScopeType};

	let options = Options::from_args();

	let input: Box<Read> = match options.input_path {
		Some(path) => Box::new(File::open(path).expect("Failed to open input file.")),
		None => Box::new(stdin())
	};
	let input_reader = BufReader::new(input);

	let mut output: Box<Write> = match options.output_path {
		Some(path) => Box::new(File::create(path).expect("Failed to create output file.")),
		None => Box::new(stdout())
	};

	let mut writer = Writer::new(&mut output);

	let mut value_changes: Vec<ValueChange> = input_reader.lines().filter_map(|line| {
		ValueChange::from_str(line.unwrap().as_str()).ok()
	}).collect();
	value_changes.sort_by_key(|v| v.timestamp);

	let mut id_iter = 0u32..93u32;
	// maps signal name -> (type, size, id)
	// TODO: make sure types of veriables don't change (i.e. someone uses 'A' as a scalar, but then later uses it as a real)
	let mut variables: HashMap<String, (VarType, usize, IdCode)> = HashMap::new();

	for elem in &value_changes {
		variables.entry(elem.signal_name.clone()).or_insert_with(|| { //TODO: get rid of the clone of every lookup
			let (sig_type, width) = match elem.value {
				value_change::Value::Scalar(_) => (VarType::Wire, 1),
				value_change::Value::BinaryVector{width, ..} => (VarType::Integer, width),
				value_change::Value::Real(_) => (VarType::Real, 32)
			};
			let id = id_iter.next().expect("Input has too many variables, ran out of ids.");
			(sig_type, width, IdCode::from(id))
		});
	}

	//TODO: nested variable scopes based on name
	let scope = Scope {
		scope_type: ScopeType::Module,
		identifier: String::from("outputs"),
		// TODO: order alphabetically?
		children: variables.iter().map(|(name, (var_type, size, code))| {
			ScopeItem::Var(Var {
				var_type: *var_type,
				size: *size as u32,
				code: *code,
				reference: name.clone()
			})
		}).collect()
	};

	let header = Header {
		comment: None,
		date: None,
		version: None,
		timescale: Some((options.step_size, options.unit)),
		items: vec![ScopeItem::Scope(scope)]
	};

	writer.header(&header).unwrap();
	writer.timestamp(0).unwrap();

	// TODO: Initial values = x

	// TODO: merge identical timestamps
	for change in value_changes {
		writer.timestamp(change.timestamp).unwrap();
		let (_, _, id) = variables.get(&change.signal_name).unwrap();
		match change.value {
			value_change::Value::Scalar(v) => {
				writer.change_scalar(*id, v).unwrap();
			},
			value_change::Value::BinaryVector{value, ..} => {
				let value: Vec<vcd::Value> = value.iter().map(|el| {
					let v: vcd::Value = el.clone().into(); // TODO: fix this
					v
				}).collect();
				writer.change_vector(*id, &value[..]).unwrap();
			},
			value_change::Value::Real(v) => {
				writer.change_real(*id, v).unwrap();
			}
		}
	}

	output.flush().unwrap();
}
