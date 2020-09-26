use anyhow::{anyhow, Context, Result};
use clap::Clap;
use nom::{
    bytes,
    error::{Error, ErrorKind},
    number, Err, IResult,
};
use std::convert::TryFrom;
use std::convert::TryInto;
use std::fs::{read, File};
use std::net::Ipv4Addr;

#[derive(Clap)]
struct Opts {
    path: std::path::PathBuf,
    #[clap(short = 'a', long = "set-addr")]
    set_ip: Option<Ipv4Addr>,
    #[clap(short = 'n', long = "set-name", default_value = "Imposter")]
    set_name: String,
}

fn len_prefixed_string(input: &[u8]) -> IResult<&[u8], String> {
    let (input, len) = number::complete::u8(input)?;
    let (remaining, str_bytes) = bytes::complete::take(len)(input)?;
    let out = String::from_utf8(str_bytes.to_owned())
        .map_err(|_| Err::Error(Error::new(input, ErrorKind::Verify)))?;
    Ok((remaining, out))
}

fn parse_region_info<'a>(input: &[u8]) -> IResult<&[u8], RegionInfo> {
    let (input, version) = number::complete::le_u32::<&[u8], ()>(input)
        .map_err(|_| Err::Error(Error::new(input, ErrorKind::Eof)))?;
    let (input, name) = len_prefixed_string(input)?;
    let (input, to_ping) = len_prefixed_string(input)?;
    let (input, servers) =
        nom::multi::length_count(number::complete::le_u32, parse_server_info)(input)?;

    Ok((
        input,
        RegionInfo {
            version,
            name,
            to_ping,
            servers,
        },
    ))
}

fn parse_server_info(input: &[u8]) -> IResult<&[u8], ServerInfo> {
    let (input, name) = len_prefixed_string(input)?;
    let (input, ip_bytes) = bytes::complete::take(4u8)(input)?;
    let ip = [ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]].into();
    let (input, port) = number::complete::le_u16::<&[u8], ()>(input)
        .map_err(|_| Err::Error(Error::new(input, ErrorKind::Eof)))?;
    let (input, _) = number::complete::le_u32::<&[u8], ()>(input)
        .map_err(|_| Err::Error(Error::new(input, ErrorKind::Eof)))?;

    Ok((input, ServerInfo { name, ip, port }))
}

fn main() -> Result<()> {
    let opts: Opts = Opts::parse();

    let file_contents = read(&opts.path).context("Unable to open file")?;
    let (_, mut region_info) =
        parse_region_info(&file_contents).map_err(|e| anyhow!("Parsing failed: {}", e))?;
    if let Some(ip) = opts.set_ip {
        region_info.servers = vec![ServerInfo {
            name: format!("{}-Master-1", opts.set_name),
            ip,
            port: 22023,
        }];
        region_info.name = opts.set_name;
        region_info.to_ping = ip.to_string();
        let mut out_file = File::create(&opts.path).context("Failed to overwrite file")?;
        region_info
            .serialize(&mut out_file)
            .context("Failed to write to file")?;
    }
    println!("{:#?}", region_info);
    Ok(())
}

#[derive(Debug)]
struct RegionInfo {
    version: u32,
    name: String,
    to_ping: String,
    servers: Vec<ServerInfo>,
}

impl RegionInfo {
    fn serialize(&self, writer: &mut dyn std::io::Write) -> Result<()> {
        writer.write_all(self.version.to_le_bytes().as_ref())?;
        writer.write_all(&[self
            .name
            .len()
            .try_into()
            .context("Too long value for name")?])?;
        writer.write_all(self.name.as_bytes())?;
        writer.write_all(&[self
            .to_ping
            .len()
            .try_into()
            .context("Too long value for to_ping")?])?;
        writer.write_all(self.to_ping.as_bytes())?;
        writer.write_all(
            u32::try_from(self.servers.len())
                .context("To many servers for u32")?
                .to_le_bytes()
                .as_ref(),
        )?;
        for server in self.servers.iter() {
            server.serialize(writer)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct ServerInfo {
    name: String,
    ip: Ipv4Addr,
    port: u16,
}

impl ServerInfo {
    fn serialize(&self, writer: &mut dyn std::io::Write) -> Result<()> {
        writer.write_all(&[self.name.len().try_into().context("Too long of a name")?])?;
        writer.write_all(self.name.as_bytes())?;
        writer.write_all(self.ip.octets().as_ref())?;
        writer.write_all(self.port.to_le_bytes().as_ref())?;
        writer.write_all(0u32.to_le_bytes().as_ref())?;
        Ok(())
    }
}
