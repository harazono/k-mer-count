extern crate bio;
extern crate rdxsort;

//use std::io;
use std::env;
//use std::fs;
use std::fs::File;
//use std::str::from_utf8;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::collections::hash_map::DefaultHasher;
use sha2::{Sha256, Sha512, Digest};

use bio::io::fastq::Reader as fqReader;
use bio::io::fastq::Record as fqRecord;
use bio::io::fasta::Reader as faReader;
use bio::io::fasta::Record as faRecord;
use crate::bio::io::fastq::FastqRead;
use crate::bio::io::fasta::FastaRead;

use rdxsort::*;
use bit_reverse::ParallelReverse;
use anyhow::Result;
use flate2::read::MultiGzDecoder;

use voracious_radix_sort::{RadixSort};

use rand::prelude::*;


const L_LEN: usize = 27;
const R_LEN: usize = 27;
const TOW_SQ20: u128 = 2_u128.pow(20);
const BLOOMFILTER_TABLE_SIZE: usize = 73 * 1024 * 1024 * 1024;//u64なら足りるはず。2^6 < 73 < 2^7で、2^37におさまる。
//const BLOOMFILTER_TABLE_SIZE: usize = 10 * 1024 * 1024;

const SIMPLE_ITTR: [u32;4] = [
    0b00000000,//AAAA
    0b01010101,//CCCC
    0b10101010,//GGGG
    0b11111111//TTTT
];

fn is_complex(sequence: &u128) -> bool {
    let mut result: bool = false;
    for i in 0..(L_LEN + R_LEN) {
        for j in SIMPLE_ITTR{
            result |= ((sequence >> i * 2) as u32 & j) == j;
        }
    }
    return result;
}

fn count_occurence(input_sequence: &Vec<u128>) -> Vec<u128>{
    let mut current_sequence: &u128 = &input_sequence[0];
    let mut ret_vec: Vec<u128> = Vec::new();
    let mut index: usize = 0;
    let mut counter: u128 = 1;
    let mut buf: u128;
    let goal: usize = input_sequence.len();
    loop{
        index += 1;
        if index >= goal{
            break ret_vec;
        }
        //in case that index reaches the border between different sequence.
        if input_sequence[index] != *current_sequence{
            let dna_string = String::from_utf8(decode_u128_2_dna_seq(&current_sequence).to_vec()).unwrap();
            if counter > TOW_SQ20{
                eprintln!("count_occurence unexpected situation: {} appears more than {}", dna_string, TOW_SQ20);
                eprintln!("{}\t{:?}", counter, dna_string);
                break ret_vec;
            }
            if true{
/*
                println!("counter: {}", counter);
                println!("current_sequence: {}", dna_string);
                println!("counter:          {:#0130b}", counter);//バイナリ列で表示する
*/
                counter = counter << (L_LEN + R_LEN) * 2;
/*
                println!("counter(shifted): {:#0130b}", counter);//バイナリ列で表示する
                println!("current_sequence: {:#0130b}", current_sequence);//バイナリ列で表示する
*/
                buf = counter + *current_sequence;
/*
                println!("sum of them:      {:#0130b}",buf);//バイナリ列で表示する
                println!("{}", decode_u128_2_occurence(&buf));
                println!("{}", String::from_utf8(decode_u128_2_dna_seq(&buf).to_vec()).unwrap());
                println!();
*/
                ret_vec.push(buf);
                counter = 1;
                current_sequence = &input_sequence[index];
            }else{
                eprintln!("is complex");
            }
        }else{
            counter += 1;
        }
    }
}

fn encode_dna_seq_2_u64(sequence: &[u8]) -> u64{
    let mut result: u64 = 0;
    for each_base in sequence.iter(){
        match each_base{
            b'A' => {result |= 0;}
            b'C' => {result |= 1;}
            b'G' => {result |= 2;}
            b'T' => {result |= 3;}
            _   => {panic!("Unexpected character: {}", each_base);}
        }
        result = result << 2;
    }
    result = result >> 2;
    return result;
}

fn encode_dna_seq_2_u128(sequence: &[u8]) -> u128{
    let mut result: u128 = 0;
    for each_base in sequence.iter(){
        match each_base{
            b'A' => {result |= 0;}
            b'C' => {result |= 1;}
            b'G' => {result |= 2;}
            b'T' => {result |= 3;}
            _   => {panic!("Unexpected character: {}", each_base);}
        }
        result = result << 2;
    }
    result = result >> 2;
    return result;
}



fn decode_u64_2_dna_seq(source:u64, index: usize, length: usize) ->u8{
    let mut result: u8 = 0;
    let mut tmp: u64 = source << (64 - length * 2);
    tmp = tmp << index * 2;
    tmp = tmp >> 62;
    match tmp{
        0 => {result = b'A';}
        1 => {result = b'C';}
        2 => {result = b'G';}
        3 => {result = b'T';}
        _ => {panic!("Never reached!!!tmp: {}", tmp);}
    }
    return result;
}

fn decode_u128_2_dna_seq(source:&u128) -> [u8; L_LEN + R_LEN]{
    let mut result: [u8; L_LEN + R_LEN] = [b'X'; L_LEN + R_LEN];
    let mut tmp: u128 = source.clone() << 20;
    let mut base: u128;
    for i in 0..L_LEN + R_LEN{
        base = (tmp & 0xC000_0000_0000_0000_0000_0000_0000_0000) >> 126;
        //println!("{}, {:#130b}", base, tmp);
        tmp = tmp << 2;
        match base{
            0 => {result[i] = b'A';}
            1 => {result[i] = b'C';}
            2 => {result[i] = b'G';}
            3 => {result[i] = b'T';}
            _ => {panic!("Never reached!!!tmp: {}", tmp);}
        }
    }
    return result;
}
fn decode_u128_2_occurence(source: &u128) -> u32{
    return TryFrom::try_from(source >> (L_LEN + R_LEN) * 2).unwrap();
}

fn extract_occurence(source: &Vec<u128>) -> Vec<u32>{
    let mut result: Vec<u32> = Vec::new();
    let length = source.len();
    let mut cnt: usize = 0;
    loop{
        if cnt < length{
            result.push(decode_u128_2_occurence(&source[cnt]));
            cnt += 1;
        }else{
            break;
        }
    }
    return result;
}

pub fn open_with_gz<P: AsRef<Path>>(p: P) -> Result<Box<dyn BufRead>> {
    let r = std::fs::File::open(p.as_ref())?;
    let ext = p.as_ref().extension();

    if ext == Some(std::ffi::OsStr::new("gz")) {
        let gz = MultiGzDecoder::new(r)?;
        let buf_reader = BufReader::new(gz);
        Ok(Box::new(buf_reader))
    } else {
        let buf_reader = BufReader::new(r);
        Ok(Box::new(buf_reader))
    }
}

fn counting_bloom_filter(path: &str) -> [u8; BLOOMFILTER_TABLE_SIZE]{
    let mut window_start: usize;
    let mut l_start: usize;
    let mut l_end:   usize;
    let mut r_start: usize;
    let mut r_end:   usize;
    let mut m_len:   usize;
    let mut loop_cnt:usize = 0;
    let mut ret_array: [u8; BLOOMFILTER_TABLE_SIZE] = [0; BLOOMFILTER_TABLE_SIZE];

    let file = File::open(path).expect("Error during opening the file");
    let mut reader = faReader::new(file);
    let mut record = faRecord::new();
    let mut buf: u64 = 0;
    let mut lr_string: [u8;L_LEN + R_LEN] = [64; L_LEN + R_LEN];

    loop {
        reader.read(&mut record).unwrap();
        if record.is_empty(){
            break;
        }
        eprintln!("loop: {:09?}, current record id:{:?}\tlength: {:?}", loop_cnt, record.id(), record.seq().len());
        loop_cnt += 1;
        for dna_chunk_size in 80..141 {
            window_start = 0;
            loop{
                m_len = dna_chunk_size - L_LEN - R_LEN;
                l_start = window_start;
                l_end   = l_start + L_LEN;
                r_start = l_end + m_len;
                r_end   = r_start + R_LEN;
                window_start += 1;

                if r_end > record.seq().len(){
                    break;
                }
                let l = &record.seq()[l_start..l_end];
                let r = &record.seq()[r_start..r_end];
                for i in 0..L_LEN{
                    lr_string[i] = l[i];
                }
                for i in 0..R_LEN{
                    lr_string[i + L_LEN] = r[i];
                }
                let table_indice:[u64;8] = hasher(&lr_string);
                let tmp: u8 = count_occurence_from_counting_bloomfilter_table(&ret_array, &lr_string);
                if rand::random::<u8>() < (u8::MAX >> tmp) && tmp != u8::MAX{
                    for i in 0..8{
                        ret_array[table_indice[i] as usize] += 1;
                    }
                }
            }
        }
    }
    return ret_array;
}

fn count_occurence_from_counting_bloomfilter_table(counting_bloomfilter_table: &[u8; BLOOMFILTER_TABLE_SIZE], query: &[u8;L_LEN + R_LEN]) -> u8{
    let indice: [u64; 8] = hasher(query);
    let mut retval: u8 = u8::MAX;
    for index in indice{
        if counting_bloomfilter_table[index as usize] < retval{
            retval = counting_bloomfilter_table[index as usize];
        }
    }
    return retval;
}



fn hasher(source: &[u8;L_LEN + R_LEN]) -> [u64; 8]{
    let mut ret_val: [u64; 8] = [0;8];
    let mut hasher = Sha512::new();
    hasher.update(source);
    let result = hasher.finalize();
    let sha512_bit_array = result.as_slice();//&[u8;64]
    for i in 0..8{
        for j in 0..8{
            ret_val[i] += sha512_bit_array[i * 8 + j] as u64;
            ret_val[i] <<= 8;
        }
    }
    return ret_val;
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = &args[1];
    eprintln!("input {:?}", path);
    //let mut reader = fastq::Reader::new(open_with_gz(path).unwrap());
    let file = File::open(path).expect("Error during opening the file");
    let mut reader = faReader::new(file);
    let mut record = faRecord::new();

    eprintln!("loading {:?} done", path);

    let mut lr_chunk:Vec<u128> = Vec::new();
    let mut window_start: usize;
    let mut l_start: usize;
    let mut l_end:   usize;
    let mut r_start: usize;
    let mut r_end:   usize;
    let mut m_len:   usize;
    let mut loop_cnt:usize = 0;

    counting_bloom_filter(path);
/*
    loop {
        reader.read(&mut record).unwrap();
        if record.is_empty(){
            break;
        }
        eprintln!("loop: {:09?}, current record id:{:?}\tlength: {:?}", loop_cnt, record.id(), record.seq().len());
        loop_cnt += 1;
        for dna_chunk_size in 80..141 {
            window_start = 0;
            loop{
                m_len = dna_chunk_size - L_LEN - R_LEN;
                l_start = window_start;
                l_end   = l_start + L_LEN;
                r_start = l_end + m_len;
                r_end   = r_start + R_LEN;
                window_start += 1;

                if r_end > record.seq().len(){
                    break;
                }
                let l = &record.seq()[l_start..l_end];
                let r = &record.seq()[r_start..r_end];
                let mut lr_string: [u8;L_LEN + R_LEN] = [64; L_LEN + R_LEN];
                for i in 0..L_LEN{
                    lr_string[i] = l[i];
                }
                for i in 0..R_LEN{
                    lr_string[i + L_LEN] = r[i];
                }
                let lr_u128 = encode_dna_seq_2_u128(&lr_string);
                lr_chunk.push(lr_u128);
            }
        }
    }
/*
1/10 or 1/100のリードを調べて当たりをつける
hyper log log counter......?
bloom filterで出現回数の少ないものをカットする
*/
    eprintln!("sort start");
    lr_chunk.voracious_mt_sort(8);
    eprintln!("sort end");
    eprintln!("count start");
    let mut sorted_lr_chunk: Vec<u128> = count_occurence(&lr_chunk);
    eprintln!("count end");
    eprintln!("sort start");
    sorted_lr_chunk.voracious_mt_sort(8);
    eprintln!("sort end");
    sorted_lr_chunk.reverse();

    for each_chunk in sorted_lr_chunk.iter() {
        let dna_string = String::from_utf8(decode_u128_2_dna_seq(each_chunk).to_vec()).unwrap();
        let occurrence = decode_u128_2_occurence(&each_chunk);
        println!("{}\t{}", occurrence, dna_string);
    }
/*
    let occurences: Vec<u32> = extract_occurence(&sorted_lr_chunk);
    for each_occurrence in occurences.iter() {
        println!("{}", each_occurrence);
    }
*/
*/
}