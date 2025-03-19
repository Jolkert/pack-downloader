use std::path::Path;

use clap::Parser;
use reqwest::header::Entry;

type DynError = Box<dyn std::error::Error + Sync + Send>;

#[tokio::main]
async fn main() -> Result<(), DynError>
{
	env_logger::init();
	let args = Args::parse();

	let result = run(&args.out_dir).await;
	if let Err(error) = &result
	{
		log::error!("A fatal error has occured! {error}");
	}

	result
}

async fn run(out_dir: &Path) -> Result<(), DynError>
{
	let current_dir = std::env::current_dir()?;
	for dir_result in std::fs::read_dir(current_dir)?.filter_map(|result| {
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
				log::warn!("Failed to read directory entry! {err}")
			}
			Ok(dir) =>
			{
				// by this point `dir` should be guaranteed to be a directory if i wrote the above filter_map right -morgan 2025-03-18
			}
		}
	}

	Ok(())
}

#[derive(Debug, clap::Parser)]
struct Args
{
	#[arg(short, long)]
	out_dir: Box<Path>,
}
