use std::env;
use std::io;
use std::path::PathBuf;

use rust_virtio_gpu::waydroid::WaydroidVenusConfig;

fn usage() {
    eprintln!(
        "Usage: waydroid-venus [--server PATH] [--rendernode PATH] [--icd PATH] [--config PATH] [--init-env PATH] [--prop PATH]... [--start] [--setup]"
    );
}

fn main() -> io::Result<()> {
    let mut config = WaydroidVenusConfig::default();
    let mut start = false;
    let mut setup = false;

    let mut args = env::args().skip(1).peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--server" => config.server = PathBuf::from(next_value(&mut args, "--server")?),
            "--rendernode" => {
                config.render_node = Some(PathBuf::from(next_value(&mut args, "--rendernode")?))
            }
            "--icd" => {
                config.vulkan_icd = Some(PathBuf::from(next_value(&mut args, "--icd")?))
            }
            "--config" => {
                config.config_session = PathBuf::from(next_value(&mut args, "--config")?)
            }
            "--init-env" => {
                config.init_environ_rc = Some(PathBuf::from(next_value(&mut args, "--init-env")?))
            }
            "--prop" => config
                .waydroid_props
                .push(PathBuf::from(next_value(&mut args, "--prop")?)),
            "--setup" => setup = true,
            "--start" => start = true,
            "--help" | "-h" => {
                usage();
                return Ok(());
            }
            other => {
                usage();
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown argument: {other}"),
                ));
            }
        }
    }

    if setup {
        config.setup()?;
        println!("Waydroid Venus configuration installed.");
    }

    if start {
        let mut child = config.spawn_server()?;
        println!("virgl_test_server ready at {}", config.socket_host.display());
        let status = child.wait()?;
        if !status.success() {
            return Err(io::Error::other(format!("virgl_test_server exited with {status}")));
        }
    }

    if !setup && !start {
        usage();
    }

    Ok(())
}

fn next_value<I>(args: &mut I, flag: &str) -> io::Result<String>
where
    I: Iterator<Item = String>,
{
    args.next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("missing value for {flag}"),
        )
    })
}
