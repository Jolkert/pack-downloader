mod errors;
mod launcher_profiles;

use std::{
	collections::{HashMap, hash_map},
	fmt::Display,
	io,
	path::{Path, PathBuf},
};

use clap::Parser;
use errors::{BadUrlError, HomeNotFoundError, MissingInstallerError, MissingPackInfoError};
use reqwest::Response;
use tokio::task::JoinSet;

type DynError = Box<dyn std::error::Error + Sync + Send>;

#[tokio::main]
async fn main() -> Result<(), DynError>
{
	unsafe {
		std::env::set_var("RUST_LOG", "INFO");
	}
	env_logger::init();
	let args = Args::parse();

	let result = run_no_gui(args).await;

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

async fn run_no_gui(args: Args) -> Result<(), DynError>
{
	let pwd = std::env::current_dir()?.canonicalize()?;
	let minecraft_dir = get_mc_dir()?.canonicalize()?;

	log::info!("Reading packinfo.toml");
	let pack_info = PackInfo::read_from_path(
		&pwd.read_dir()?
			.find_map(|result| {
				result.ok().and_then(|entry| {
					(entry.file_name() == "packinfo.toml").then(|| pwd.join(entry.file_name()))
				})
			})
			.ok_or(MissingPackInfoError)?,
	)?;

	if minecraft_dir.join("versions").read_dir()?.any(|entry| {
		entry.is_ok_and(|file| {
			file.path().file_stem().is_some_and(|file_name| {
				file_name
					.to_str()
					.is_some_and(|file_name_str| file_name_str == pack_info.forge_version)
			})
		})
	})
	{
		log::info!("Found correct Forge version. Skipping Forge install");
	}
	else
	{
		log::info!("Correct Forge version not found. Running Forge installer");
		install_forge(&args.forge_installer)?;
	}

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

	for dir_result in std::fs::read_dir(&pwd)?.filter_map(|result| {
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

	let mut old_version_paths = Vec::new();
	if let Some(mod_list) = try_read_to_string(pwd.join("mods.toml"))?
		.map(|string| toml::from_str::<HashMap<String, String>>(&string))
		.transpose()?
	{
		let installed_pack_mods = try_read_to_string(format!("{}.toml", pack_info.name))?
			.map(|toml_str| toml::from_str::<InstalledPack>(&toml_str))
			.transpose()?
			.map(|pack| pack.mod_list)
			.unwrap_or_default();

		let join_set = mod_list
			.into_iter()
			.filter_map(|(mod_name, url)| {
				if let Some(old_file_name) = installed_pack_mods.get(&mod_name)
				{
					old_version_paths.push(out_dir.join("mods").join(old_file_name));
					url.matches(old_file_name.to_str().unwrap())
						.next()
						.is_some()
						.then(|| tokio::task::spawn(download_file(url)))
				}
				else
				{
					Some(tokio::task::spawn(download_file(url)))
				}
			})
			.collect::<JoinSet<_>>();

		let join_results = join_set.join_all().await;
		for join_result in &join_results
		{
			match join_result
			{
				Ok(Ok(_)) => (),
				Ok(Err(err)) => log::error!("Failed to download file! {err}"),
				Err(err) => log::error!("Failed to join thread! {err}"),
			}
		}

		if join_results
			.iter()
			.all(|result| result.as_ref().is_ok_and(|inner| inner.is_ok()))
		{
			for path in old_version_paths
			{
				std::fs::remove_file(path)?;
			}
			for file in std::fs::read_dir("./.mod_installer_temp")?
			{
				let file = file?;
				std::fs::copy(file.path(), out_dir.join("mods").join(file.file_name()))?;
			}
		}
		else
		{
			log::error!("Could not download all mods! Aborting mods download!");
		}

		std::fs::remove_dir_all("./.mod_installer_temp/")?;
	}
	else
	{
		log::warn!("No mods.toml file found!");
	}

	Ok(())
}

fn try_read_to_string(path: impl AsRef<Path>) -> std::io::Result<Option<String>>
{
	std::fs::exists(&path)?
		.then(|| std::fs::read_to_string(&path))
		.transpose()
}

async fn download_file(url: String) -> Result<String, DynError>
{
	let file_name =
		urlencoding::decode(url.split('/').next_back().ok_or(BadUrlError)?.trim())?.into_owned();

	log::info!("Downloading {file_name}");

	let bytes = reqwest::get(url).await?.bytes().await?;
	let path = PathBuf::from("./.mod_installer_temp").join(&file_name);
	tokio::fs::write(path, bytes).await?;

	Ok(file_name)
}

fn install_forge(installer_path: &Path) -> Result<(), DynError>
{
	log::info!("Beginning forge install");
	if std::process::Command::new("java")
		.arg("-jar")
		.arg(installer_path.to_str().ok_or(MissingInstallerError)?)
		.spawn()?
		.wait()?
		.success()
	{
		log::info!("Successfully installed forge");
	}
	else
	{
		log::warn!("Failed to install forge")
	}

	Ok(())
}

fn get_mc_dir() -> Result<PathBuf, DynError>
{
	let home_dir = homedir::my_home()?.ok_or(HomeNotFoundError)?;
	let ret = append_minecraft(home_dir);
	log::info!("Found minecraft directory at {}", ret.to_string_lossy());
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct InstalledPack
{
	pack_info: PackInfo,
	mod_list: HashMap<String, PathBuf>,
}

#[derive(Debug, clap::Parser)]
pub struct Args
{
	#[arg(short, long)]
	pub forge_installer: PathBuf,

	#[arg(short, long)]
	pub out_dir: Option<PathBuf>,
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
