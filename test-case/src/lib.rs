use anyhow::{anyhow, bail, Result};
use clap::{Parser, Subcommand};
use outbound_redis::RedisParameter;
use spin_http::{Method, Request, Response};
use std::{
    env, error, fmt,
    fs::{self, File},
    io, iter,
    time::SystemTime,
};

wit_bindgen_rust::export!("../wit/spin-http.wit");

struct SpinHttp;

impl spin_http::SpinHttp for SpinHttp {
    fn handle_http_request(request: Request) -> Response {
        //eprintln!("handle request: {}", request.uri);
        if request.method != Method::Post {
            Response {
                status: 405,
                headers: None,
                body: None,
            }
        } else if request.uri == "/" {
            dispatch(request.body)
        } else if request.uri != "/foo" {
            Response {
                status: 404,
                headers: None,
                body: None,
            }
        } else if request.headers != [("foo".into(), "bar".into())]
            || request.body.as_deref() != Some(b"Hello, SpinHttp!")
        {
            Response {
                status: 400,
                headers: None,
                body: None,
            }
        } else {
            Response {
                status: 200,
                headers: Some(vec![("lorem".into(), "ipsum".into())]),
                body: Some("dolor sit amet".as_bytes().to_owned()),
            }
        }
    }
}

wit_bindgen_rust::export!("../wit/spin-redis.wit");

struct SpinRedis;

impl spin_redis::SpinRedis for SpinRedis {
    fn handle_redis_message(_body: Vec<u8>) -> Result<(), spin_redis::Error> {
        Ok(())
    }
}

wit_bindgen_rust::import!("../wit/spin-config.wit");

impl fmt::Display for spin_config::Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Provider(provider_err) => write!(f, "provider error: {provider_err}"),
            Self::InvalidKey(invalid_key) => write!(f, "invalid key: {invalid_key}"),
            Self::InvalidSchema(invalid_schema) => write!(f, "invalid schema: {invalid_schema}"),
            Self::Other(other) => write!(f, "other: {other}"),
        }
    }
}

impl error::Error for spin_config::Error {}

wit_bindgen_rust::import!("../wit/wasi-outbound-http.wit");

impl fmt::Display for wasi_outbound_http::HttpError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Self::Success => "success",
            Self::DestinationNotAllowed => "destination not allowed",
            Self::InvalidUrl => "invalid url",
            Self::RequestError => "request error",
            Self::RuntimeError => "runtime error",
            Self::TooManyRequests => "too many requests",
        })
    }
}

impl error::Error for wasi_outbound_http::HttpError {}

wit_bindgen_rust::import!("../wit/outbound-redis.wit");

impl fmt::Display for outbound_redis::Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Self::Success => "success",
            Self::Error => "redis error",
        })
    }
}

impl error::Error for outbound_redis::Error {}

wit_bindgen_rust::import!("../wit/outbound-pg.wit");

impl fmt::Display for outbound_pg::PgError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Success => f.write_str("success"),
            Self::ConnectionFailed(message) => write!(f, "connection failed: {message}"),
            Self::BadParameter(message) => write!(f, "bad parameter: {message}"),
            Self::QueryFailed(message) => write!(f, "query failed: {message}"),
            Self::ValueConversionFailed(message) => write!(f, "value conversion failed: {message}"),
            Self::OtherError(message) => write!(f, "error: {message}"),
        }
    }
}

impl error::Error for outbound_pg::PgError {}

wit_bindgen_rust::import!("../wit/outbound-mysql.wit");

impl fmt::Display for outbound_mysql::MysqlError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Success => f.write_str("success"),
            Self::ConnectionFailed(message) => write!(f, "connection failed: {message}"),
            Self::BadParameter(message) => write!(f, "bad parameter: {message}"),
            Self::QueryFailed(message) => write!(f, "query failed: {message}"),
            Self::ValueConversionFailed(message) => write!(f, "value conversion failed: {message}"),
            Self::OtherError(message) => write!(f, "error: {message}"),
        }
    }
}

impl error::Error for outbound_mysql::MysqlError {}

wit_bindgen_rust::import!("../wit/key-value.wit");

impl fmt::Display for key_value::Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::StoreTableFull => f.write_str("store table full"),
            Self::NoSuchStore => f.write_str("no such store"),
            Self::AccessDenied => f.write_str("access denied"),
            Self::InvalidStore => f.write_str("invalid store"),
            Self::NoSuchKey => f.write_str("no such key"),
            Self::Io(message) => write!(f, "io error: {message}"),
        }
    }
}

impl error::Error for key_value::Error {}

fn parse_pg(param: &str) -> Result<outbound_pg::ParameterValue> {
    use outbound_pg::ParameterValue as PV;

    Ok(if param == "null" {
        PV::DbNull
    } else {
        let (type_, value) = param
            .split_once(':')
            .ok_or_else(|| anyhow!("expected ':' in {param}"))?;

        match type_ {
            "boolean" => PV::Boolean(value.parse()?),
            "int8" => PV::Int8(value.parse()?),
            "int16" => PV::Int16(value.parse()?),
            "int32" => PV::Int32(value.parse()?),
            "int64" => PV::Int64(value.parse()?),
            "uint8" => PV::Uint8(value.parse()?),
            "uint16" => PV::Uint16(value.parse()?),
            "uint32" => PV::Uint32(value.parse()?),
            "uint64" => PV::Uint64(value.parse()?),
            "floating32" => PV::Floating32(value.parse()?),
            "floating64" => PV::Floating64(value.parse()?),
            "str" => PV::Str(value),
            "binary" => PV::Binary(value.as_bytes()),
            _ => bail!("unknown parameter type: {type_}"),
        }
    })
}

fn parse_mysql(param: &str) -> Result<outbound_mysql::ParameterValue> {
    use outbound_mysql::ParameterValue as PV;

    Ok(if param == "null" {
        PV::DbNull
    } else {
        let (type_, value) = param
            .split_once(':')
            .ok_or_else(|| anyhow!("expected ':' in {param}"))?;

        match type_ {
            "boolean" => PV::Boolean(value.parse()?),
            "int8" => PV::Int8(value.parse()?),
            "int16" => PV::Int16(value.parse()?),
            "int32" => PV::Int32(value.parse()?),
            "int64" => PV::Int64(value.parse()?),
            "uint8" => PV::Uint8(value.parse()?),
            "uint16" => PV::Uint16(value.parse()?),
            "uint32" => PV::Uint32(value.parse()?),
            "uint64" => PV::Uint64(value.parse()?),
            "floating32" => PV::Floating32(value.parse()?),
            "floating64" => PV::Floating64(value.parse()?),
            "str" => PV::Str(value),
            "binary" => PV::Binary(value.as_bytes()),
            _ => bail!("unknown parameter type: {type_}"),
        }
    })
}

#[derive(Debug, Parser)]
#[clap(author, version, about)]
pub struct Cli {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Config {
        key: String,
    },
    Http {
        url: String,
    },
    RedisPublish {
        address: String,
        key: String,
        value: String,
    },
    RedisSet {
        address: String,
        key: String,
        value: String,
    },
    RedisGet {
        address: String,
        key: String,
    },
    RedisIncr {
        address: String,
        key: String,
    },
    RedisDel {
        address: String,
        keys: Vec<String>,
    },
    RedisSadd {
        address: String,
        key: String,
        params: Vec<String>,
    },
    RedisSrem {
        address: String,
        key: String,
        params: Vec<String>,
    },
    RedisSmembers {
        address: String,
        key: String,
    },
    RedisExecute {
        address: String,
        command: String,
        params: Vec<String>,
    },
    PostgresExecute {
        address: String,
        statement: String,
        params: Vec<String>,
    },
    PostgresQuery {
        address: String,
        statement: String,
        params: Vec<String>,
    },
    MysqlExecute {
        address: String,
        statement: String,
        params: Vec<String>,
    },
    MysqlQuery {
        address: String,
        statement: String,
        params: Vec<String>,
    },
    KeyValueOpen {
        name: String,
    },
    KeyValueGet {
        store: u32,
        key: String,
    },
    KeyValueSet {
        store: u32,
        key: String,
        value: String,
    },
    KeyValueDelete {
        store: u32,
        key: String,
    },
    KeyValueExists {
        store: u32,
        key: String,
    },
    KeyValueGetKeys {
        store: u32,
    },
    KeyValueClose {
        store: u32,
    },
    WasiEnv {
        key: String,
    },
    WasiEpoch,
    WasiRandom,
    WasiStdio,
    WasiRead {
        file_name: String,
    },
    WasiReaddir {
        dir_name: String,
    },
    WasiStat {
        file_name: String,
    },
}

fn dispatch(body: Option<Vec<u8>>) -> Response {
    match execute(body) {
        Ok(()) => Response {
            status: 200,
            headers: None,
            body: None,
        },

        Err(e) => Response {
            status: 500,
            headers: None,
            body: Some(format!("{e:?}").into_bytes()),
        },
    }
}

fn execute(body: Option<Vec<u8>>) -> Result<()> {
    let body = body.ok_or_else(|| anyhow!("empty request body"))?;
    let commands = iter::once("<wasm module>")
        .chain(serde_json::from_slice::<Vec<&str>>(&body)?)
        .collect::<Vec<_>>();

    match &Cli::try_parse_from(commands)?.command {
        Command::Config { key } => {
            spin_config::get_config(key)?;
        }

        Command::Http { url } => {
            use wasi_outbound_http::{Method, Request};

            wasi_outbound_http::request(Request {
                method: Method::Get,
                uri: url,
                headers: &[],
                params: &[],
                body: None,
            })?;
        }

        Command::RedisPublish {
            address,
            key,
            value,
        } => {
            outbound_redis::publish(address, key, value.as_bytes())?;
        }

        Command::RedisSet {
            address,
            key,
            value,
        } => {
            outbound_redis::set(address, key, value.as_bytes())?;
        }

        Command::RedisGet { address, key } => {
            outbound_redis::get(address, key)?;
        }

        Command::RedisIncr { address, key } => {
            outbound_redis::incr(address, key)?;
        }

        Command::RedisDel { address, keys } => {
            outbound_redis::del(
                address,
                &keys.iter().map(String::as_str).collect::<Vec<_>>(),
            )?;
        }

        Command::RedisSadd {
            address,
            key,
            params,
        } => {
            outbound_redis::sadd(
                address,
                key,
                &params.iter().map(String::as_str).collect::<Vec<_>>(),
            )?;
        }

        Command::RedisSmembers { address, key } => {
            outbound_redis::smembers(address, key)?;
        }

        Command::RedisSrem {
            address,
            key,
            params,
        } => {
            outbound_redis::srem(
                address,
                key,
                &params.iter().map(String::as_str).collect::<Vec<_>>(),
            )?;
        }

        Command::RedisExecute {
            address,
            command,
            params,
        } => {
            outbound_redis::execute(
                address,
                command,
                &params
                    .iter()
                    .map(|s| RedisParameter::Binary(s.as_bytes()))
                    .collect::<Vec<_>>(),
            )?;
        }

        Command::PostgresExecute {
            address,
            statement,
            params,
        } => {
            outbound_pg::execute(
                address,
                statement,
                &params
                    .iter()
                    .map(|param| parse_pg(param))
                    .collect::<Result<Vec<_>>>()?,
            )?;
        }

        Command::PostgresQuery {
            address,
            statement,
            params,
        } => {
            outbound_pg::query(
                address,
                statement,
                &params
                    .iter()
                    .map(|param| parse_pg(param))
                    .collect::<Result<Vec<_>>>()?,
            )?;
        }

        Command::MysqlExecute {
            address,
            statement,
            params,
        } => {
            outbound_mysql::execute(
                address,
                statement,
                &params
                    .iter()
                    .map(|param| parse_mysql(param))
                    .collect::<Result<Vec<_>>>()?,
            )?;
        }

        Command::MysqlQuery {
            address,
            statement,
            params,
        } => {
            outbound_mysql::query(
                address,
                statement,
                &params
                    .iter()
                    .map(|param| parse_mysql(param))
                    .collect::<Result<Vec<_>>>()?,
            )?;
        }

        Command::KeyValueOpen { name } => {
            key_value::open(name)?;
        }

        Command::KeyValueGet { store, key } => {
            key_value::get(*store, key)?;
        }

        Command::KeyValueSet { store, key, value } => {
            key_value::set(*store, key, value.as_bytes())?;
        }

        Command::KeyValueDelete { store, key } => {
            key_value::delete(*store, key)?;
        }

        Command::KeyValueExists { store, key } => {
            key_value::exists(*store, key)?;
        }

        Command::KeyValueGetKeys { store } => {
            key_value::get_keys(*store)?;
        }

        Command::KeyValueClose { store } => {
            key_value::close(*store);
        }

        Command::WasiEnv { key } => print!("{}", env::var(key)?),

        Command::WasiEpoch => print!(
            "{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_millis()
        ),

        Command::WasiRandom => {
            let mut buffer = [0u8; 8];
            getrandom::getrandom(&mut buffer).map_err(|_| anyhow!("getrandom error"))?;
        }

        Command::WasiStdio => {
            io::copy(&mut io::stdin().lock(), &mut io::stdout().lock())?;
        }

        Command::WasiRead { file_name } => {
            io::copy(&mut File::open(file_name)?, &mut io::stdout().lock())?;
        }

        Command::WasiReaddir { dir_name } => {
            let mut comma = false;
            for entry in fs::read_dir(dir_name)? {
                if comma {
                    print!(",");
                } else {
                    comma = true;
                }

                print!(
                    "{}",
                    entry?
                        .file_name()
                        .to_str()
                        .ok_or_else(|| anyhow!("non-UTF-8 file name"))?
                );
            }
        }

        Command::WasiStat { file_name } => {
            let metadata = fs::metadata(file_name)?;
            print!(
                "length:{},modified:{}",
                metadata.len(),
                metadata
                    .modified()?
                    .duration_since(SystemTime::UNIX_EPOCH)?
                    .as_millis()
            );
        }
    }

    Ok(())
}
