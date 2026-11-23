//! Minimal command-line parsing shared by the binaries.
//!
//! A full argument parser would be the heaviest dependency of the workspace, and the
//! handful of flags used here does not justify it.

use std::collections::VecDeque;
use std::fmt::Display;
use std::num::{NonZeroU64, NonZeroUsize};
use std::str::FromStr;
use std::time::Duration;

use crate::server::ServerConfig;

/// Result of parsing: the error is a message for the user.
pub type Result<T> = std::result::Result<T, String>;

/// Queue of command-line items.
pub struct Args {
    items: VecDeque<String>,
}

impl Args {
    /// Takes the arguments of the process, program name excluded.
    pub fn from_env() -> Self {
        Self::new(std::env::args().skip(1))
    }

    /// Wraps the given items.
    pub fn new(items: impl IntoIterator<Item = String>) -> Self {
        Self { items: items.into_iter().collect() }
    }

    /// Returns the next flag. `--flag=value` is split, `value` becomes the next item.
    pub fn next_flag(&mut self) -> Option<String> {
        let item = self.items.pop_front()?;
        if let Some((flag, value)) = item.split_once('=') {
            if flag.starts_with("--") {
                self.items.push_front(value.to_string());
                return Some(flag.to_string());
            }
        }

        Some(item)
    }

    /// Takes the value of `flag`.
    pub fn value(&mut self, flag: &str) -> Result<String> {
        self.items.pop_front().ok_or_else(|| format!("{flag} requires a value"))
    }

    /// Takes the value of `flag` and converts it with `parse`.
    pub fn parse_with<T>(
        &mut self,
        flag: &str,
        parse: impl FnOnce(&str) -> Result<T>,
    ) -> Result<T> {
        let value = self.value(flag)?;
        parse(&value).map_err(|err| format!("invalid value for {flag}: {err}"))
    }

    /// Takes the value of `flag` and parses it with [`FromStr`].
    pub fn parse<T: FromStr>(&mut self, flag: &str) -> Result<T>
    where
        T::Err: Display,
    {
        self.parse_with(flag, |value| value.parse::<T>().map_err(|err| err.to_string()))
    }
}

/// Parses a duration with a unit: `250us`, `1.5ms`, `2s`, `1m`; `0` needs no unit.
pub fn parse_duration(text: &str) -> Result<Duration> {
    let text = text.trim();
    if text == "0" {
        return Ok(Duration::ZERO);
    }

    let unit_start = text.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(text.len());
    let (number, unit) = text.split_at(unit_start);
    let value: f64 = number.parse().map_err(|_| format!("'{text}' is not a duration"))?;
    let nanos_per_unit = match unit.trim() {
        "ns" => 1.0,
        "us" | "µs" => 1e3,
        "ms" => 1e6,
        "s" => 1e9,
        "m" | "min" => 60e9,
        "" => return Err(format!("'{text}' has no unit (ns, us, ms, s, m)")),
        other => return Err(format!("unknown time unit '{other}'")),
    };
    if !value.is_finite() || value < 0.0 {
        return Err(format!("'{text}' is not a valid duration"));
    }

    Ok(Duration::from_nanos((value * nanos_per_unit).round() as u64))
}

/// Parses a byte count with an optional binary suffix: `4096`, `64K`, `1M`, `2GiB`.
pub fn parse_size(text: &str) -> Result<u64> {
    let text = text.trim();
    let text = text
        .strip_suffix("iB")
        .or_else(|| text.strip_suffix("ib"))
        .or_else(|| text.strip_suffix('B'))
        .or_else(|| text.strip_suffix('b'))
        .unwrap_or(text);
    let (number, shift) = match text.chars().last() {
        Some('K' | 'k') => (&text[..text.len() - 1], 10),
        Some('M' | 'm') => (&text[..text.len() - 1], 20),
        Some('G' | 'g') => (&text[..text.len() - 1], 30),
        Some('T' | 't') => (&text[..text.len() - 1], 40),
        _ => (text, 0),
    };
    let value: u64 = number.trim().parse().map_err(|_| format!("'{text}' is not a size"))?;

    value
        .checked_shl(shift)
        .filter(|v| v >> shift == value)
        .ok_or_else(|| format!("'{text}' overflows"))
}

/// Parses a non-zero byte count or number.
pub fn parse_nonzero_usize(text: &str) -> Result<NonZeroUsize> {
    let value = parse_size(text)?;
    usize::try_from(value)
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or_else(|| format!("'{text}' must be a positive number"))
}

/// Parses a non-zero byte count or rate.
pub fn parse_nonzero_u64(text: &str) -> Result<NonZeroU64> {
    NonZeroU64::new(parse_size(text)?).ok_or_else(|| format!("'{text}' must be positive"))
}

/// Parses a byte count that has to fit into 32 bits.
pub fn parse_u32_size(text: &str) -> Result<u32> {
    u32::try_from(parse_size(text)?).map_err(|_| format!("'{text}' does not fit into 32 bits"))
}

/// Parses a byte value: `171` or `0xAB`.
pub fn parse_byte(text: &str) -> Result<u8> {
    let text = text.trim();
    let parsed = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => u8::from_str_radix(hex, 16),
        None => text.parse(),
    };

    parsed.map_err(|_| format!("'{text}' is not a byte value"))
}

/// Help text of the flags handled by [`apply_server_flag`].
pub const SERVER_FLAGS_HELP: &str = "  --export <path>        Path clients mount (default: /)
  --latency <dur>        Backend response time of every procedure, e.g. 200us, 1.5ms (default: 0)
  --jitter <dur>         Half-width of the uniform jitter added to --latency (default: 0)
  --bandwidth <bytes/s>  Throughput of the simulated device shared by READ/WRITE, e.g. 2G
                         (default: unlimited)
  --concurrency <n>      VFS workers, i.e. the backend queue depth (default: 64)
  --buf-size <bytes>     Size of one allocator buffer (default: 1M)
  --buf-count <n>        Number of allocator buffers (default: 64)
  --max-io <bytes>       READ/WRITE size advertised through FSINFO (default: 1M)
  --file-size <bytes>    Size of every regular file (default: 1G)
  --files <n>            Regular files listed in every directory (default: 128)
  --dirs <n>             Subdirectories listed in every directory (default: 8)
  --fill <byte>          Fill READ buffers with this byte, e.g. 0xAB (default: leave as is)";

/// Applies `flag` to `config` when it is one of the server flags.
///
/// # Returns
///
/// `Ok(true)` when the flag was consumed, `Ok(false)` when it is not a server flag.
pub fn apply_server_flag(config: &mut ServerConfig, flag: &str, args: &mut Args) -> Result<bool> {
    match flag {
        "--export" => config.export = args.value(flag)?,
        "--latency" => config.vfs.latency.base = args.parse_with(flag, parse_duration)?,
        "--jitter" => config.vfs.latency.jitter = args.parse_with(flag, parse_duration)?,
        "--bandwidth" => {
            config.vfs.latency.bandwidth = Some(args.parse_with(flag, parse_nonzero_u64)?)
        }
        "--concurrency" => config.concurrency = args.parse_with(flag, parse_nonzero_usize)?,
        "--buf-size" => config.buf_size = args.parse_with(flag, parse_nonzero_usize)?,
        "--buf-count" => config.buf_count = args.parse_with(flag, parse_nonzero_usize)?,
        "--max-io" => config.vfs.max_io = args.parse_with(flag, parse_u32_size)?,
        "--file-size" => config.vfs.file_size = args.parse_with(flag, parse_size)?,
        "--files" => config.vfs.files_per_dir = args.parse(flag)?,
        "--dirs" => config.vfs.dirs_per_dir = args.parse(flag)?,
        "--fill" => config.vfs.read_fill = Some(args.parse_with(flag, parse_byte)?),
        _ => return Ok(false),
    }

    Ok(true)
}

/// Formats a duration for humans: `250us`, `1.50ms`, `2.00s`.
pub fn format_duration(duration: Duration) -> String {
    format_micros(duration.as_secs_f64() * 1e6)
}

/// Formats a number of microseconds for humans.
pub fn format_micros(micros: f64) -> String {
    if micros < 1_000.0 {
        format!("{micros:.0}us")
    } else if micros < 1_000_000.0 {
        format!("{:.2}ms", micros / 1_000.0)
    } else {
        format!("{:.2}s", micros / 1_000_000.0)
    }
}

/// Formats a byte count with a binary unit.
pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Installs a `tracing` subscriber writing to stderr.
///
/// `RUST_LOG` wins when set; otherwise `default_filter` applies. The binaries default
/// to `warn`, so the per-request debug logs of the server do not distort measurements.
pub fn init_tracing(default_filter: &str) {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter));
    let _ =
        tracing_subscriber::fmt().with_env_filter(filter).with_writer(std::io::stderr).try_init();
}

/// Formats a byte count the short way when it is a whole number of binary units:
/// `4K`, `64K`, `1M`, otherwise `4097B`.
pub fn format_size_short(bytes: u64) -> String {
    for (shift, unit) in [(40, 'T'), (30, 'G'), (20, 'M'), (10, 'K')] {
        if bytes > 0 && bytes % (1u64 << shift) == 0 {
            return format!("{}{unit}", bytes >> shift);
        }
    }
    format!("{bytes}B")
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{parse_byte, parse_duration, parse_size, Args};

    #[test]
    fn durations_need_a_unit_except_zero() {
        assert_eq!(parse_duration("0"), Ok(Duration::ZERO));
        assert_eq!(parse_duration("250us"), Ok(Duration::from_micros(250)));
        assert_eq!(parse_duration("1.5ms"), Ok(Duration::from_micros(1500)));
        assert_eq!(parse_duration("2s"), Ok(Duration::from_secs(2)));
        assert_eq!(parse_duration("1m"), Ok(Duration::from_secs(60)));
        assert!(parse_duration("10").is_err());
        assert!(parse_duration("10parsecs").is_err());
        assert!(parse_duration("-1ms").is_err());
    }

    #[test]
    fn sizes_accept_binary_suffixes() {
        assert_eq!(parse_size("4096"), Ok(4096));
        assert_eq!(parse_size("64K"), Ok(64 << 10));
        assert_eq!(parse_size("64k"), Ok(64 << 10));
        assert_eq!(parse_size("1M"), Ok(1 << 20));
        assert_eq!(parse_size("2GiB"), Ok(2 << 30));
        assert_eq!(parse_size("1TB"), Ok(1 << 40));
        assert!(parse_size("lots").is_err());
        assert!(parse_size("99999999999G").is_err());
    }

    #[test]
    fn short_sizes_use_whole_units() {
        assert_eq!(super::format_size_short(4096), "4K");
        assert_eq!(super::format_size_short(1 << 20), "1M");
        assert_eq!(super::format_size_short(3 << 30), "3G");
        assert_eq!(super::format_size_short(4097), "4097B");
    }

    #[test]
    fn bytes_accept_decimal_and_hex() {
        assert_eq!(parse_byte("171"), Ok(0xAB));
        assert_eq!(parse_byte("0xAB"), Ok(0xAB));
        assert!(parse_byte("256").is_err());
    }

    #[test]
    fn flags_with_equals_sign_are_split() {
        let mut args = Args::new(["--latency=1ms", "--files", "3"].map(String::from));

        assert_eq!(args.next_flag().as_deref(), Some("--latency"));
        assert_eq!(args.value("--latency").as_deref(), Ok("1ms"));
        assert_eq!(args.next_flag().as_deref(), Some("--files"));
        assert_eq!(args.parse::<usize>("--files"), Ok(3));
        assert_eq!(args.next_flag(), None);
        assert!(args.value("--files").is_err());
    }
}
