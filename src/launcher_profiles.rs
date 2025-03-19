use std::{collections::HashMap, path::Path};

use serde_json as json;

use crate::{DynError, PackInfo, errors::BadOutDirError};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct LauncherProfilesObject
{
	pub profiles: HashMap<String, LauncherProfile>,
	pub settings: json::Value,
	pub version: json::Value,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct LauncherProfile
{
	pub name: String,
	pub icon: String,
	#[serde(rename = "lastVersionId")]
	pub last_version_id: String,
	#[serde(rename = "type")]
	pub version_type: String,
	pub created: String,
	#[serde(rename = "lastUsed")]
	pub last_used: String,
	#[serde(rename = "javaArgs")]
	pub java_args: Option<String>,
	#[serde(rename = "gameDir")]
	pub game_dir: Option<String>,
	#[serde(rename = "javaDir")]
	pub java_dir: Option<String>,
	#[serde(rename = "logConfig")]
	pub log_config: Option<String>,
	#[serde(rename = "logConfigIsXML")]
	pub config_is_xml: Option<bool>,
	pub resolution: Option<json::Value>,
}

const VALID_LAUNCHER_PROFILE_FILES: [&str; 2] = [
	"launcher_profiles.json",
	"launcher_profiles_microsoft_store.json",
];
pub fn create_profiles(
	minecraft_dir: &Path,
	pack_info: &PackInfo,
	out_dir: &Path,
) -> Result<(), DynError>
{
	for file_name in VALID_LAUNCHER_PROFILE_FILES.iter()
	{
		let profiles_file_path = minecraft_dir.join(file_name);

		if std::fs::exists(&profiles_file_path)?
		{
			create_launcher_profile(&profiles_file_path, pack_info, out_dir)?
		}
	}

	Ok(())
}

fn create_launcher_profile(
	profiles_path: &Path,
	pack_info: &PackInfo,
	out_dir: &Path,
) -> Result<(), DynError>
{
	let mut profiles_object =
		json::from_str::<LauncherProfilesObject>(&std::fs::read_to_string(profiles_path)?)?;

	let profile_id = pack_info.to_string();
	if !profiles_object.profiles.contains_key(&profile_id)
	{
		let new_profile = LauncherProfile {
			name: profile_id.clone(),
			icon: String::from("SOUL_SAND"),
			last_version_id: pack_info.forge_version.clone(),
			version_type: String::from("custom"),
			created: chrono::Utc::now().to_rfc3339(),
			last_used: chrono::Utc::now().to_rfc3339(),
			java_args: Some(String::from(
				"-Xmx6G -XX:+UnlockExperimentalVMOptions -XX:+UseG1GC -XX:G1NewSizePercent=20 -XX:G1ReservePercent=20 -XX:MaxGCPauseMillis=50 -XX:G1HeapRegionSize=32M",
			)),
			game_dir: Some(String::from(
				out_dir.as_os_str().to_str().ok_or(BadOutDirError)?,
			)),
			java_dir: None,
			log_config: None,
			config_is_xml: None,
			resolution: None,
		};

		profiles_object.profiles.insert(profile_id, new_profile);
	}

	Ok(std::fs::write(
		profiles_path,
		json::to_string_pretty(&profiles_object)?,
	)?)
}
