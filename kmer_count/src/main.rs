extern crate bio;
extern crate rdxsort;
extern crate getopts;
use getopts::Options;
use std::fs::File;
use std::io::Write;
use std::{env, process};
use voracious_radix_sort::{RadixSort};
use kmer_count::counting_bloomfilter_util::L_LEN;
use kmer_count::counting_bloomfilter_util::R_LEN;
use kmer_count::counting_bloomfilter_util::BLOOMFILTER_TABLE_SIZE;
use kmer_count::counting_bloomfilter_util::{build_counting_bloom_filter, number_of_high_occurence_kmer, pick_up_high_occurence_kmer};



fn decode_u128_2_dna_seq(source:&u128, char_size: usize) -> Vec<u8>{
    let mut result: Vec<u8> = Vec::new();
    let mut base;
    for i in 0..char_size{
        base = source >> 2 * (char_size - 1 - i) & 3;
        match base{
            0 => {result.push(b'A');}
            1 => {result.push(b'C');}
            2 => {result.push(b'G');}
            3 => {result.push(b'T');}
            _ => {panic!("Never reached!!!base: {}", base);}
        }
    }
    return result;
}

fn decode_u128_l(source: &u128) -> [u8; L_LEN]{
    let mut result: [u8; L_LEN] = [b'X'; L_LEN];
    let mut base;
    for i in 0..L_LEN{
        base = source >> ((R_LEN + L_LEN - i - 1) * 2) & 3;
        match base{
            0 => {result[i] = b'A';}
            1 => {result[i] = b'C';}
            2 => {result[i] = b'G';}
            3 => {result[i] = b'T';}
            _ => {panic!("Never reached!!!base: {}", base);}
        }
    }
    return result;
}

fn decode_u128_r(source: &u128) -> [u8; R_LEN]{
    let mut result: [u8; R_LEN] = [b'X'; R_LEN];
    let mut base;
    for i in 0..R_LEN{
        base = source >> ((R_LEN - i - 1) * 2) & 3;
        match base{
            0 => {result[i] = b'A';}
            1 => {result[i] = b'C';}
            2 => {result[i] = b'G';}
            3 => {result[i] = b'T';}
            _ => {panic!("Never reached!!!base: {}", base);}
        }
    }
    return result;
}

fn print_usage(program: &str, opts: &Options) {
    let brief = format!("Usage: {} FILE [options]", program);
    print!("{}", opts.usage(&brief));
    process::exit(0);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.optopt("o", "output", "set output file name", "NAME");
    opts.optopt("t", "thread", "number of threads to use for radix sort. default value is 8.", "THREAD");
    opts.optopt("a", "threshold", "threshold for hyper log counter. default value is 8.", "THRESHOLD");
    opts.optflag("r", "only-num", "outputs only total number of k-mer");
    opts.optflag("h", "help", "print this help menu");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => { m }
        Err(f) => { panic!("{}", f.to_string()) }
    };
    if matches.opt_present("h") {
        print_usage(&program, &opts);
        return;
    }

    let input_file = if !matches.free.is_empty() {
        matches.free[0].clone()
    } else {
        print_usage(&program, &opts);
        return;
    };

    let threads = if matches.opt_present("t") {
        matches.opt_str("t").unwrap().parse::<usize>().unwrap()
    }else{
        8
    };

    let threshold = if matches.opt_present("a") {
        matches.opt_str("a").unwrap().parse::<u64>().unwrap()
    }else{
        8
    };

    let output_file = if matches.opt_present("o") {
        matches.opt_str("o").unwrap()
    }else{
        format!("{:?}_threshold{}_threads{}.out", input_file, threshold, threads)
    };


    eprintln!("input  file: {:?}",  input_file);
    eprintln!("loading {:?} done", input_file);

    //1??????
    eprintln!("start calling build_counting_bloom_filter");
    let counting_bloom_filter_table: Box<[u64; BLOOMFILTER_TABLE_SIZE]> = build_counting_bloom_filter(&input_file);
    eprintln!("finish calling build_counting_bloom_filter");

    //2??????
    eprintln!("start calling number_of_high_occurence_kmer");
    let (high_occr_bloomfilter_table, occurence) = number_of_high_occurence_kmer(&counting_bloom_filter_table, &input_file, threshold);
    eprintln!("finish calling number_of_high_occurence_kmer");
    //3??????

    eprintln!("Vec size is {}", occurence);
    eprintln!("start calling pick_up_high_occurence_kmer");
    let occr_with_mergin = ((occurence as f64) * 1.2).ceil() as usize;
    let mut high_occurence_kmer: Vec<u128> = pick_up_high_occurence_kmer(&high_occr_bloomfilter_table, &input_file, occr_with_mergin);
    eprintln!("finish calling pick_up_high_occurence_kmer");

    //sort??????
    eprintln!("start voracious_mt_sort({})", threads);
    high_occurence_kmer.voracious_mt_sort(threads);
    eprintln!("finish voracious_mt_sort({})", threads);

/*
    let mut previous_l_kmer: [u8; L_LEN] = [b'A'; L_LEN];
    let mut current_l_kmer:  [u8; L_LEN] = [b'A'; L_LEN];
    for each_kmer in high_occurence_kmer{
        current_l_kmer = decode_u128_l(&each_kmer);
        if current_l_kmer != previous_l_kmer{
            println!("{:?}", String::from_utf8(current_l_kmer.to_vec()).unwrap());
        }
        previous_l_kmer = current_l_kmer;
    }
*/
    eprintln!("start writing to output file: {:?}", &output_file);

    let mut w = File::create(&output_file).unwrap();
    let mut previous_kmer: u128 = 0;
    let mut cnt = 0;

    if matches.opt_present("r") {
        for each_kmer in high_occurence_kmer{
            if previous_kmer != each_kmer{
                cnt += 1;
            }
            previous_kmer = each_kmer;
        }
        writeln!(&mut w, "k-mer count: {}\tthreshold: {}\tinput file {:?}", cnt, threshold, &output_file).unwrap();
    }else{
        for each_kmer in high_occurence_kmer{
            if previous_kmer != each_kmer{
                writeln!(&mut w, "{:?}", String::from_utf8(decode_u128_2_dna_seq(&each_kmer, 54)).unwrap()).unwrap();
            }
            previous_kmer = each_kmer;
        }
    }


    eprintln!("finish writing to output file: {:?}", &output_file);
    eprintln!("total cardinarity of 54-mer: {}", cnt);
}