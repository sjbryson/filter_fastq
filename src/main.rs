use clap::Parser;
use flate2::read::MultiGzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::collections::HashSet;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};

#[derive(Parser, Debug)]
#[command(author, version, about = "FASTQ pair filtering by Read ID")]
struct Args {
    /// Read 1 FASTQ file (can be .gz)
    #[arg(short = '1', long)]
    r1: String,

    /// Read 2 FASTQ file (can be .gz)
    #[arg(short = '2', long)]
    r2: String,

    /// Text file with read IDs to filter
    #[arg(short = 'f', long)]
    filter: Option<String>,

    /// Output prefix
    #[arg(short = 'o', long)]
    out_prefix: String,

    /// Keep only records in the filter list
    #[arg(long, conflicts_with = "exclude")]
    keep: bool,

    /// Exclude records in the filter list (default)
    #[arg(long, conflicts_with = "keep")]
    exclude: bool,

    /// Compress output as fastq.gz
    #[arg(long)]
    gz: bool,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    /// 1. Load Filter IDs (from File or Stdin)
    let filter_ids = load_filter_ids(args.filter)?;

    /// 2. Open Input Readers
    let r1 = open_fastq(&args.r1)?;
    let r2 = open_fastq(&args.r2)?;

    /// 3. Prepare Output Writers
    let ext = if args.gz { "fastq.gz" } else { "fastq" };
    let mut out_r1 = open_writer(&format!("{}_R1.{}", args.out_prefix, ext), args.gz)?;
    let mut out_r2 = open_writer(&format!("{}_R2.{}", args.out_prefix, ext), args.gz)?;

    /// 4. Execute optimized filter path
    if args.keep {
        filter_keep(r1, r2, &mut out_r1, &mut out_r2, &filter_ids)?;
    } else {
        filter_exclude(r1, r2, &mut out_r1, &mut out_r2, &filter_ids)?;
    }

    Ok(())
}


/// KEEP MODE: Only write records present in filter_ids. Stop early if all found.
fn filter_keep<R: BufRead, W: Write>(
    r1: R, 
    r2: R, 
    mut w1: W, 
    mut w2: W,
    filter_ids: &HashSet<String>
) -> io::Result<()> {
    let mut r1_lines = r1.lines();
    let mut r2_lines = r2.lines();
    let mut found_count = 0;
    let mut total_processed = 0;
    let total_to_find = filter_ids.len();

    while found_count < total_to_find {
        let (Some(rec1), Some(rec2)) = (read_fastq_record(&mut r1_lines)?, read_fastq_record(&mut r2_lines)?) 
            else { break; };
        
        total_processed += 1;
        if filter_ids.contains(&clean_id(&rec1[0])) {
            write_record(&mut w1, &rec1)?;
            write_record(&mut w2, &rec2)?;
            found_count += 1;
        }
    }
    report_summary(found_count, total_to_find, total_processed);
    Ok(())
}

/// EXCLUDE MODE: Write records NOT in filter_ids. Fast-path once all targets excluded.
fn filter_exclude<R: BufRead, W: Write>(
    r1: R, 
    r2: R, 
    mut w1: W, 
    mut w2: W,
    filter_ids: &HashSet<String>
) -> io::Result<()> {
    let mut r1_lines = r1.lines();
    let mut r2_lines = r2.lines();
    let mut found_count = 0;
    let mut total_processed = 0;
    let total_to_find = filter_ids.len();

    loop {
        let (Some(rec1), Some(rec2)) = (read_fastq_record(&mut r1_lines)?, read_fastq_record(&mut r2_lines)?) 
            else { break; };

        total_processed += 1;
        if found_count < total_to_find && filter_ids.contains(&clean_id(&rec1[0])) {
            found_count += 1;
            continue; 
        }

        write_record(&mut w1, &rec1)?;
        write_record(&mut w2, &rec2)?;
    }
    report_summary(found_count, total_to_find, total_processed);
    Ok(())
}

fn clean_id(id: &str) -> String {
    id.trim_start_matches('@')
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_end_matches("/1")
        .trim_end_matches("/2")
        .to_string()
}

fn load_filter_ids(path: Option<String>) -> io::Result<HashSet<String>> {
    let mut ids = HashSet::new();
    let reader: Box<dyn BufRead> = match path {
        Some(p) => Box::new(BufReader::new(File::open(p)?)),
        None => Box::new(BufReader::new(io::stdin())),
    };
    for line in reader.lines() {
        let l = line?;
        let id = clean_id(&l);
        if !id.is_empty() { ids.insert(id); }
    }
    Ok(ids)
}

fn open_fastq(path: &str) -> io::Result<BufReader<Box<dyn Read>>> {
    let file = File::open(path)?;
    if path.ends_with(".gz") {
        Ok(BufReader::new(Box::new(MultiGzDecoder::new(file))))
    } else {
        Ok(BufReader::new(Box::new(file)))
    }
}

fn open_writer(path: &str, gz: bool) -> io::Result<BufWriter<Box<dyn Write>>> {
    let file = File::create(path)?;
    if gz {
        Ok(BufWriter::new(Box::new(GzEncoder::new(file, Compression::default()))))
    } else {
        Ok(BufWriter::new(Box::new(file)))
    }
}

fn read_fastq_record<I>(lines: &mut I) -> io::Result<Option<Vec<String>>>
where
    I: Iterator<Item = io::Result<String>>,
{
    let mut record = Vec::with_capacity(4);
    for i in 0..4 {
        match lines.next() {
            Some(line) => record.push(line?),
            None => return if i == 0 { Ok(None) } else { Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Malformed FASTQ")) },
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

fn report_summary(found: usize, total: usize, processed: usize) {
    eprintln!("\n--- Process Summary ---");
    eprintln!("Total records scanned: {}", processed);
    eprintln!("IDs matched: {} of {}", found, total);
    if found < total {
        eprintln!("WARNING: {} IDs from the filter list were not found.", total - found);
    }
}