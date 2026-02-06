
use clap::Parser;
use flate2::read::MultiGzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::collections::HashSet;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write, Read};

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// read 1 FASTQ file
    #[arg(short = '1', long)]
    r1: String,

    /// read 2 FASTQ file
    #[arg(short = '2', long)]
    r2: String,

    /// txt file with read IDs to filter (one per line)
    #[arg(short = 'f', long)]
    filter: String,

    /// output prefix
    #[arg(short = 'o', long)]
    out_prefix: String,

    /// keep records in filter list instead of excluding
    #[arg(long)]
    invert: bool,

    /// write to fastq.gz instead of fastq
    #[arg(long)]
    gz: bool,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let filter_ids = load_filter_ids(&args.filter)?;

    let r1 = open_fastq(&args.r1)?;
    let r2 = open_fastq(&args.r2)?;

    let ext = if args.gz { "fastq.gz" } else { "fastq" };

    let out_r1 = open_writer(
        &format!("{}_R1.{}", args.out_prefix, ext),
        args.gz,
    )?;

    let out_r2 = open_writer(
        &format!("{}_R2.{}", args.out_prefix, ext),
        args.gz,
    )?;

    filter_fastq_pairs(
        r1,
        r2,
        out_r1,
        out_r2,
        &filter_ids,
        args.invert,
    )?;

    Ok(())
}

fn open_fastq(path: &str) -> io::Result<BufReader<Box<dyn Read>>> {
    let file = File::open(path)?;

    if path.ends_with(".gz") {
        let decoder = MultiGzDecoder::new(file);
        Ok(BufReader::new(Box::new(decoder)))
    } else {
        Ok(BufReader::new(Box::new(file)))
    }
}

fn open_writer(path: &str, gz: bool) -> io::Result<BufWriter<Box<dyn Write>>> {
    let file = File::create(path)?;

    if gz {
        let encoder = GzEncoder::new(file, Compression::default());
        Ok(BufWriter::new(Box::new(encoder)))
    } else {
        Ok(BufWriter::new(Box::new(file)))
    }
}

fn load_filter_ids(path: &str) -> io::Result<HashSet<String>> {
    let reader = BufReader::new(File::open(path)?);
    let mut ids = HashSet::new();

    for line in reader.lines() {
        let id = line?.trim_start_matches('@').to_string();
        ids.insert(id);
    }

    Ok(ids)
}

fn extract_id(header: &str) -> String {
    header
        .trim_start_matches('@')
        .split_whitespace()
        .next()
        .unwrap()
        .to_string()
}

fn filter_fastq_pairs<R: BufRead, W: Write>(
    r1: R,
    r2: R,
    mut w1: W,
    mut w2: W,
    filter_ids: &HashSet<String>,
    invert: bool,
) -> io::Result<()> {
    let mut r1_lines = r1.lines();
    let mut r2_lines = r2.lines();

    loop {
        let rec1 = read_fastq_record(&mut r1_lines)?;
        let rec2 = read_fastq_record(&mut r2_lines)?;

        match (rec1, rec2) {
            (Some(r1_rec), Some(r2_rec)) => {
                let id = extract_id(&r1_rec[0]);
                let in_filter = filter_ids.contains(&id);

                let keep = if invert { in_filter } else { !in_filter };

                if keep {
                    write_record(&mut w1, &r1_rec)?;
                    write_record(&mut w2, &r2_rec)?;
                }
            }
            (None, None) => break,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "R1 and R2 FASTQ files differ in length",
                ));
            }
        }
    }

    Ok(())
}

fn read_fastq_record<I>(lines: &mut I) -> io::Result<Option<Vec<String>>>
where
    I: Iterator<Item = io::Result<String>>,
{
    let mut record = Vec::with_capacity(4);

    for i in 0..4 {
        match lines.next() {
            Some(line) => record.push(line?),
            None => {
                if i == 0 {
                    // true EOF
                    return Ok(None);
                } else {
                    // mid-record EOF -> corrupt FASTQ
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Incomplete FASTQ record",
                    ));
                }
            }
        }
    }

    Ok(Some(record))
}

fn write_record<W: Write>(writer: &mut W, record: &[String]) -> io::Result<()> {
    for line in record {
        writeln!(writer, "{line}")?;
    }
    Ok(())
}