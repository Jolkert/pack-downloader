mod errors;
mod launcher_profiles;

use std::{
	fmt::Display,
	io,
	path::{Path, PathBuf},
};

use clap::Parser;
use errors::{ForgeInstallError, HomeNotFoundError, MissingInstallerError, MissingPackInfoError};

type DynError = Box<dyn std::error::Error>;

fn main() -> Result<(), DynError>
{
	unsafe {
		std::env::set_var("RUST_LOG", "INFO");
	}
	env_logger::init();
	let args = Args::parse();

	let result = run(args);
	if let Err(error) = &result
	{
		log::error!("A fatal error has occured! {error}");
	}
	else
	{
		log::info!("Successfully installed!");
	}

	result
}

fn run(args: Args) -> Result<(), DynError>
{
	let pwd = std::env::current_dir()?.canonicalize()?;
	let minecraft_dir = get_mc_dir()?.canonicalize()?;

	install_forge(&args.forge_installer, &minecraft_dir)?;

	log::info!("Attemping to read packinfo.toml");
	let pack_info = PackInfo::read_from_path(
		&pwd.read_dir()?
			.find_map(|result| {
				result.ok().and_then(|entry| {
					(entry.file_name() == "packinfo.toml").then(|| pwd.join(entry.file_name()))
				})
			})
			.ok_or(MissingPackInfoError)?,
	)?;
	log::info!("Successful read of packinfo.toml");

	log::info!("Reading output directory");
	let out_dir = {
		let temp = args
			.out_dir
			.unwrap_or_else(|| minecraft_dir.join(&pack_info.name));
		std::fs::create_dir_all(&temp)?;
		temp.canonicalize()?
	};

	log::info!("Starting creation of launcher profiles");
	launcher_profiles::create_profiles(&minecraft_dir, &pack_info, &out_dir)?;

	for dir_result in std::fs::read_dir(pwd)?.filter_map(|result| {
		result.map_or_else(
			|err| Some(Err(err)),
			|entry| {
				entry.file_type().map_or_else(
					|err| Some(Err(err)),
					|file_type| file_type.is_dir().then_some(Ok(entry)),
				)
			},
		)
	})
	{
		match dir_result
		{
			Err(err) =>
			{
				log::warn!("Failed to read directory entry! {err}");
			}
			Ok(dir) =>
			{
				// by this point `dir` should be guaranteed to be a directory if i wrote the above filter_map right -morgan 2025-03-18
				recursive_copy(
					dir.path(),
					out_dir.join(dir.file_name()),
					dir.file_name() != "config",
				)?;
			}
		}
	}

	Ok(())
}

fn install_forge(installer_path: &Path, minecraft_dir: &Path) -> Result<(), DynError>
{
	log::info!("Beginning forge install");
	if std::process::Command::new("java")
		.arg("-jar")
		.arg(installer_path.to_str().ok_or(MissingInstallerError)?)
		.spawn()?
		.wait()?
		.success()
	{
		log::info!("Installed forge");
		Ok(())
	}
	else
	{
		Err(ForgeInstallError.into())
	}
}

fn get_mc_dir() -> Result<PathBuf, DynError>
{
	let home_dir = homedir::my_home()?.ok_or(HomeNotFoundError)?;
	let ret = append_minecraft(home_dir);
	log::info!("Foubd minecraft directory at {}", ret.to_string_lossy());
	Ok(ret)
}

fn append_minecraft(mut path: PathBuf) -> PathBuf
{
	if cfg!(target_os = "windows")
	{
		path.push("AppData");
		path.push("Roaming");
	}

	path.push(".minecraft");
	path
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct PackInfo
{
	name: String,
	pack_version: String,
	mc_version: String,
	forge_version: String,
}
impl Display for PackInfo
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result
	{
		write!(f, "{}-{}_{}", self.name, self.pack_version, self.mc_version)
	}
}
impl PackInfo
{
	fn read_from_path(path: &Path) -> Result<Self, DynError>
	{
		Ok(toml::from_str(&std::fs::read_to_string(path)?)?)
	}
}

#[derive(Debug, clap::Parser)]
struct Args
{
	#[arg(short, long)]
	forge_installer: PathBuf,

	#[arg(short, long)]
	out_dir: Option<PathBuf>,
}

// yoinked from https://nick.groenen.me/notes/recursively-copy-files-in-rust/
// yes i couldve written this on my own but its 5am ok im tired
// - morgan 2025-03-19
fn recursive_copy(
	source: impl AsRef<Path>,
	destination: impl AsRef<Path>,
	should_overwrite: bool,
) -> io::Result<()>
{
	std::fs::create_dir_all(&destination)?;
	for entry in std::fs::read_dir(source.as_ref())?
	{
		let entry = entry?;
		if entry.file_type()?.is_dir()
		{
			log::info!("Reading directory {}", source.as_ref().to_string_lossy());
			recursive_copy(
				entry.path(),
				destination.as_ref().join(entry.file_name()),
				should_overwrite,
			)?;
		}
		else if should_overwrite || !std::fs::exists(destination.as_ref())?
		{
			log::info!(
				"Attempting copy of {} -> {}",
				source.as_ref().to_string_lossy(),
				destination.as_ref().to_string_lossy()
			);
			std::fs::copy(entry.path(), destination.as_ref().join(entry.file_name()))?;
		}
	}

	Ok(())
}
