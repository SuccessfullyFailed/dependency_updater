use std::error::Error;

use file_ref::FileRef;



const RECURSE_ARGS:&[&str] = &["recurse", "r"];



fn main() -> Result<(), Box<dyn Error>> {

	// Figure out target dirs.
	let args:Vec<String> = std::env::args().collect();
	let root:FileRef = if args.len() > 1 && FileRef::new(&args[1]).exists() { FileRef::new(&args[1]) } else { FileRef::working_dir() };
	let mut target_dirs:Vec<FileRef> = vec![root.clone()];
	if args.iter().any(|arg| RECURSE_ARGS.contains(&arg.to_lowercase().as_str())) {
		target_dirs.extend(root.scanner().include_dirs().recurse_filter(|dir| (dir.clone() + "/Cargo.toml").exists()));
	}

	// Loop through target dirs.
	for crate_dir in target_dirs {
		let toml:FileRef = crate_dir.clone() + "/Cargo.toml";
		if !crate_dir.exists() || !toml.exists() {
			continue;
		}

		// Collect lines of dependencies in Cargo.toml.
		let mut toml_contents:String = toml.read()?;
		let dependencies:Vec<DependencyDefinition> = get_dependencies_from_toml(&toml_contents);
		for dependency in &dependencies {
			println!("{}: {:?}", dependency.name, dependency.args);
		}

		// Replace version numbers with newest versions in toml.
		let mut toml_modified:bool = false;
		for dependency in &dependencies {
			if let Some((_, current_version)) = dependency.args.iter().find(|(key, _)| key == "version") {

				// If version needs to be updated, do so.
				if let Ok(newest_version) = dependency_newest_version(dependency) {
					if &newest_version != current_version {
						let new_version:String = newest_version.trim().trim_matches('"').to_string();
						toml_contents = toml_contents.replace(&dependency.original_line, &dependency.original_line.replace(current_version, &new_version));
						toml_modified = true;
					}
				}
			}
		}

		// If toml was modified, write new contents.
		if toml_modified {
			toml.write(&toml_contents)?;
		}
	}

	// Return success.
	Ok(())
}



/// Get a list of dependency definition from toml-contents.
fn get_dependencies_from_toml(toml_contents:&str) -> Vec<DependencyDefinition> {

	// Read file and loop through lines.
	let mut category:String = String::new();
	let mut dependencies:Vec<DependencyDefinition> = Vec::new();
	for line in toml_contents.split('\n').filter(|line| !line.is_empty()).map(|line| line.split('#').next().unwrap().trim()).filter(|line| !line.is_empty()) {
		if line.starts_with('[') && line.ends_with(']') {
			category = line[1..line.len() - 1].to_string();
		} else if category.contains("dependencies") && line.contains("=") {
			let line_components:Vec<&str> = line.split('=').collect();
			if line_components.len() > 1 {

				// Get argument key, value and arguments in the value.
				let key:&str = line_components[0].trim();
				let value:&str = line[line_components[0].len() + 1..].trim();
				let mut arguments:Vec<(String, String)> = Vec::new();
				if value.starts_with('"') && value.ends_with('"') {
					arguments.push(("verison".to_string(), value.trim_matches('"').to_string()));
				} else if value.starts_with('{') && value.ends_with('}') {
					arguments.extend(get_args_from_flat_dict(&value[1..]));
				}
				dependencies.push(DependencyDefinition { name: key.trim().to_string(), args: arguments, original_line: line.to_string() });
			}
		}
	}

	// Return dependencies.
	dependencies
}

/// Get a list of keys and values from a JSON dictionary string.
fn get_args_from_flat_dict(dict_string:&str) -> Vec<(String, String)> {
	let mut arguments:Vec<(String, String)> = Vec::new();
	let mut start_index:usize = 0;
	let mut stage:u8 = 0;
	let mut current_key:String = String::new();
	let mut value_start:char = '?';
	for (char_index, loop_char) in dict_string.chars().enumerate() {

		// End of key.
		if stage == 0 && loop_char == '=' {
			current_key = dict_string[start_index..char_index].trim().trim_matches('"').to_string();
			start_index = char_index + 1;
			stage = 1;
		}

		// Start of value.
		else if stage == 1 {
			value_start = loop_char;
			stage = 2;
		}

		// End of value.
		else if stage == 2 && loop_char == value_start {
			arguments.push((current_key.to_string(), dict_string[start_index..char_index + 1].trim().trim_matches('"').to_string()));
			start_index = char_index + 1;
			stage = 3;
		}

		// Comma to next value.
		else if stage == 3 && loop_char == ',' {
			start_index = char_index + 1;
			stage = 0;
		}
	}

	// Return arguments.
	arguments
}



/// Get the newest version of a dependency.
fn dependency_newest_version(dependency:&DependencyDefinition) -> Result<String, Box<dyn Error>> {

	// Local path.
	if let Some((_, dependency_path)) = dependency.args.iter().find(|(key, _)| key == "path") {
		if let Ok(dependency_toml) = (FileRef::new(&dependency_path).absolute() + "/Cargo.toml").read() {
			if let Some(newest_version) = dependency_toml.split('\n').find(|line| line.trim().starts_with("version") && line.contains('=')).map(|line| line.split('=').skip(1).next().unwrap()) {
				return Ok(newest_version.to_string());
			}
		}
	}

	// Unable to get version.
	Err(format!("Could not get dependency version of {}.", dependency.name).into())
}


struct DependencyDefinition {
	name:String,
	args:Vec<(String, String)>,
	original_line:String
}