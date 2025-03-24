#[derive(Debug, thiserror::Error)]
#[error("Could not find .packinfo file!")]
pub struct MissingPackInfoError;

#[derive(Debug, thiserror::Error)]
#[error("Could not find forge installer file!")]
pub struct MissingInstallerError;

#[derive(Debug, thiserror::Error)]
#[error("Error installing Minecraft Forge!")]
pub struct ForgeInstallError;

#[derive(Debug, thiserror::Error)]
#[error("Could not find user home directory!")]
pub struct HomeNotFoundError;

#[derive(Debug, thiserror::Error)]
#[error("Could not encode out directory as utf-8!")]
pub struct BadOutDirError;

#[derive(Debug, thiserror::Error)]
#[error("File url not / separated!")]
pub struct BadUrlError;
