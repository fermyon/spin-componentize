use anyhow::{anyhow, bail, Result};
use clap::{Parser, Subcommand};
use spin::http_types::{Method, Request, Response};
use std::{
    env,
    fs::{self, File},
    io::{self, Write},
    iter, str,
    time::SystemTime,
};

#[macro_use]
mod wit {
    wit_bindgen::generate!({
        world: "reactor",
        path: "../wit-0.7",
        macro_call_prefix: "wit::",
    });
}
use wit::fermyon::spin::{self, postgres};
use wit::{exports::fermyon::spin as exports, fermyon::spin::mysql};

struct Spin;
export_reactor!(Spin);

impl exports::inbound_http::InboundHttp for Spin {
    fn handle_request(request: Request) -> Response {
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

impl exports::inbound_redis::InboundRedis for Spin {
    fn handle_message(_body: Vec<u8>) -> Result<(), spin::redis::Error> {
        Ok(())
    }
}

fn parse_pg(param: &str) -> Result<spin::postgres::ParameterValue> {
    use spin::postgres::ParameterValue as PV;

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

fn parse_mysql(param: &str) -> Result<spin::mysql::ParameterValue> {
    use spin::mysql::ParameterValue as PV;

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
        Ok(()) => {
            _ = io::stdout().flush();
            _ = io::stderr().flush();

            Response {
                status: 200,
                headers: None,
                body: None,
            }
        }

        Err(e) => Response {
            status: 500,
            headers: None,
            body: Some(format!("{e:?}").into_bytes()),
        },
    }
}

fn execute(body: Option<Vec<u8>>) -> Result<()> {
    let body = body.ok_or_else(|| anyhow!("empty request body"))?;
    let command = iter::once("<wasm module>")
        .chain(str::from_utf8(&body)?.split("%20"))
        .collect::<Vec<_>>();

    match Cli::try_parse_from(command)?.command {
        Command::Config { key } => {
            spin::config::get_config(&key)?;
        }

        Command::Http { url } => {
            spin::http::send_request(&Request {
                method: Method::Get,
                uri: url,
                headers: Vec::new(),
                params: Vec::new(),
                body: None,
            })?;
        }

        Command::RedisPublish {
            address,
            key,
            value,
        } => {
            spin::redis::publish(&address, &key, &value.into_bytes())?;
        }

        Command::RedisSet {
            address,
            key,
            value,
        } => {
            spin::redis::set(&address, &key, &value.into_bytes())?;
        }

        Command::RedisGet { address, key } => {
            spin::redis::get(&address, &key)?;
        }

        Command::RedisIncr { address, key } => {
            spin::redis::incr(&address, &key)?;
        }

        Command::RedisDel { address, keys } => {
            spin::redis::del(
                &address,
                &keys.iter().map(String::as_str).collect::<Vec<_>>(),
            )?;
        }

        Command::RedisSadd {
            address,
            key,
            params,
        } => {
            spin::redis::sadd(
                &address,
                &key,
                &params.iter().map(String::as_str).collect::<Vec<_>>(),
            )?;
        }

        Command::RedisSmembers { address, key } => {
            spin::redis::smembers(&address, &key)?;
        }

        Command::RedisSrem {
            address,
            key,
            params,
        } => {
            spin::redis::srem(
                &address,
                &key,
                &params.iter().map(String::as_str).collect::<Vec<_>>(),
            )?;
        }

        Command::RedisExecute {
            address,
            command,
            params,
        } => {
            let params: Vec<_> = params.into_iter().map(|s| s.into_bytes()).collect();
            spin::redis::execute(
                &address,
                &command,
                &params
                    .iter()
                    .map(|s| spin::redis_types::RedisParameter::Binary(s))
                    .collect::<Vec<_>>(),
            )?;
        }

        Command::PostgresExecute {
            address,
            statement,
            params,
        } => {
            postgres::execute(
                &address,
                &statement,
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
            postgres::query(
                &address,
                &statement,
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
            mysql::execute(
                &address,
                &statement,
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
            spin::mysql::query(
                &address,
                &statement,
                &params
                    .iter()
                    .map(|param| parse_mysql(param))
                    .collect::<Result<Vec<_>>>()?,
            )?;
        }

        Command::KeyValueOpen { name } => {
            spin::key_value::open(&name)?;
        }

        Command::KeyValueGet { store, key } => {
            spin::key_value::get(store, &key)?;
        }

        Command::KeyValueSet { store, key, value } => {
            spin::key_value::set(store, &key, value.as_bytes())?;
        }

        Command::KeyValueDelete { store, key } => {
            spin::key_value::delete(store, &key)?;
        }

        Command::KeyValueExists { store, key } => {
            spin::key_value::exists(store, &key)?;
        }

        Command::KeyValueGetKeys { store } => {
            spin::key_value::get_keys(store)?;
        }

        Command::KeyValueClose { store } => {
            spin::key_value::close(store);
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
