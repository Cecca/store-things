use anyhow::{anyhow, bail, Result};
use clap::{Arg, Command};
use sha2::Digest;
use std::fs::File;
use std::io::{prelude::*, BufReader};
use std::path::{Path, PathBuf};

const CASD_DIR: &str = ".casd";

struct Config {
    basedir: PathBuf,
}

impl Config {
    fn get() -> Result<Self> {
        for dir in std::env::current_dir()?.ancestors() {
            let cdir = dir.join(CASD_DIR);
            if cdir.is_dir() {
                return Ok(Self { basedir: cdir });
            }
        }
        Err(anyhow!(
            "Not in a CASD-managed directory. Perhaps you should run `casd init`?"
        ))
    }
}

fn hash_contents<P: AsRef<Path>>(path: P) -> Result<String> {
    fn inner(path: &Path) -> Result<String> {
        if !path.exists() {
            bail!("The path `{:?}` does not exist file", path);
        }
        if !path.is_file() {
            bail!("The path `{:?}` is not a file", path);
        }

        let mut f = BufReader::new(File::open(path)?);
        let mut hasher = sha2::Sha512::new();
        let mut buf = [0u8; 1024];
        while let Ok(cnt) = f.read(&mut buf) {
            if cnt == 0 {
                break;
            }
            hasher.update(&buf[..cnt]);
        }
        let hash = hasher.finalize();
        let hash = format!("{:x}", hash);
        Ok(hash)
    }
    inner(path.as_ref())
}

fn do_add<P: AsRef<Path>>(config: &Config, path: P) -> Result<PathBuf> {
    let hash = hash_contents(&path)?;
    let target = config.basedir.join(hash);
    if target.is_file() {
        log::info!("File {:?} already exists, skipping", target);
    } else {
        std::fs::copy(&path, &target)?;
    }

    Ok(target)
}

fn do_init() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let cdir = cwd.join(CASD_DIR);
    if !cdir.is_dir() {
        std::fs::create_dir(cdir)?;
    }
    Ok(())
}

fn main() -> Result<()> {
    let args = Command::new("casd")
        .about("CAS file manager utility")
        .subcommand_required(true)
        .subcommand(Command::new("init").about("initializes casd in the current working directory"))
        .subcommand(
            Command::new("add")
                .about("add a new entry to the cas")
                .arg(Arg::new("path").required(true)),
        )
        .get_matches();

    match args.subcommand() {
        Some(("init", _)) => {
            do_init()?;
        }
        Some(("add", matches)) => {
            let config = Config::get()?;
            let path: &String = matches.get_one("path").unwrap();
            do_add(&config, path)?;
        }
        _ => unreachable!(),
    }

    Ok(())
}
