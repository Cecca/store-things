use anyhow::{bail, Context, Result};
use clap::{Arg, Command};
use sha2::Digest;
use std::fs::File;
use std::io::{prelude::*, BufReader};
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(serde::Deserialize)]
struct Config {
    clippings: PathBuf,
    strip_dir: Option<PathBuf>,
    screenshot_dir: PathBuf,
}

impl Config {
    fn get() -> Result<Self> {
        let home = std::env::home_dir().unwrap();
        let cdir = home.join(".config").join("store-things");
        if cdir.is_dir() {
            let conf_path = cdir.join("config.toml");
            let mut f = File::open(conf_path)?;
            let mut conf_str = String::new();
            f.read_to_string(&mut conf_str)?;
            let conf: Self = toml::from_str(&conf_str)?;
            return Ok(conf);
        } else {
            bail!("no configuration found")
        }
    }

    fn get_clippings_dir(&self) -> Result<PathBuf> {
        expand_user(&self.clippings)
    }

    fn get_screenshot_dir(&self) -> Result<PathBuf> {
        expand_user(&self.screenshot_dir)
    }

    fn strip_prefix(&self, path: &PathBuf) -> Result<PathBuf> {
        if let Some(prefix) = self.strip_dir.as_ref() {
            let prefix = expand_user(prefix)?;
            if path.starts_with(&prefix) {
                let plen = prefix.to_str().context("prefix to string")?.len();
                let path = path.to_str().context("path to string")?;
                let path = PathBuf::from_str(&path[plen..])?;
                Ok(path)
            } else {
                Ok(path.clone())
            }
        } else {
            Ok(path.clone())
        }
    }
}

fn most_recent_file(dir: &PathBuf) -> Result<PathBuf> {
    let dir = expand_user(dir)?;
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&dir).context("listing directory")? {
        let path = entry?.path();
        if path.is_file() {
            files.push(path);
        }
    }
    files.sort_by_key(|path| path.metadata().unwrap().modified().unwrap());
    files.last().cloned().context("getting last file")
}

fn expand_user(path: &PathBuf) -> Result<PathBuf> {
    let home = std::env::home_dir().context("getting home dir")?;
    let path = path.to_str().context("conversion to string")?;
    let path = path.replace("~", home.to_str().context("conversion to string")?);
    PathBuf::from_str(&path).context("creation of pathbuf")
}

fn hash_contents<P: AsRef<Path>>(path: P) -> Result<String> {
    fn inner(path: &Path) -> Result<String> {
        if !path.exists() {
            bail!("The path `{:?}` does not exist", path);
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

    let extension = path
        .as_ref()
        .extension()
        .map(|ext| ext.to_str().unwrap())
        .unwrap_or("");

    let clippings_dir = config.get_clippings_dir()?;
    if !clippings_dir.is_dir() {
        std::fs::create_dir(&clippings_dir).context("creating clippings directory")?;
    }
    let mut target = clippings_dir.join(hash);
    target.set_extension(extension);

    if target.is_file() {
        log::info!("File {:?} already exists, skipping", target);
    } else {
        std::fs::copy(&path, &target)?;
    }

    std::process::Command::new("wl-copy")
        .arg(&config.strip_prefix(&target)?)
        .spawn()?
        .wait()?;

    Ok(target)
}

fn main() -> Result<()> {
    env_logger::init();

    let args = Command::new("store")
        .about("store things with unique names")
        .arg(
            Arg::new("last-screenshot")
                .long("last-screenshot")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(Arg::new("path").required(false))
        .get_matches();

    let config = Config::get()?;
    let path: String = if args.get_flag("last-screenshot") {
        let screen_dir = config.get_screenshot_dir()?;
        most_recent_file(&screen_dir)?
            .to_str()
            .context("converting to string")?
            .to_owned()
    } else {
        args.get_one::<String>("path")
            .context("you should provide a path to store")?
            .clone()
    };
    do_add(&config, path)?;

    Ok(())
}
