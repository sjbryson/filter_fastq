### Installation
Clone the repository and build with Cargo:
`cargo build --release`

The binary will be located at:
`target/release/fastq-filter`

 ### Usage
```
		fastq-filter \
			[options]
			--r1 reads_1.fastq.gz [Read 1 FASTQ file (.fastq or .fastq.gz)]\
			--r2 reads_2.fastq.gz [Read 2 FASTQ file (.fastq or .fastq.gz)]\
			-f , --filter ids.txt [Text file of read IDs to filter (one per line)]\
			-o, --out-prefix [Prefix for output files]
```
## Options

```
		--exclude <default>[Exclude reads present in the filter list]
		--keep [Keep only reads present in the filter list]
		--gz Write gzipped FASTQ output
```

 ### Output
Produces:

```  
		output_prefix_R1.fastq
		output_prefix_R2.fastq
```

Or, with gzip enabled:

 ```
		output_prefix_R1.fastq.gz
		output_prefix_R2.fastq.gz
```

### Filter File Format

- One read ID per line
- Leading @ is optional
- Read numbers "/1" or "/2" are ignored
- Anything after first whitespace is ignored

#### ID List Styles:

 ```
		@read123
		read456
		read789/1
```

#### Example Usage
Remove reads listed in bad_ids.txt:

```  
		fastq-filter \
			--r1 sample_R1.fastq.gz \
			--r2 sample_R2.fastq.gz \
			-f bad_ids.txt \
			-o cleaned \
			--exclude \
			--gz
```
  
  Keep only reads listed in good_ids.txt:

```
		fastq-filter \
			--r1 sample_R1.fastq \
			--r2 sample_R2.fastq \
			-f good_ids.txt \
			-o subset \
			--keep
```