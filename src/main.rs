
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate image;
extern crate byteorder;
extern crate rand;
extern crate num;

use std::ops::Range;

fn main()
{
	env_logger::init().unwrap();
	
	let image = load_image();
	debug!("image = {}x{}", image.width(), image.height());
	let max_time = 16.0;
	let legend = (-140.0 .. -25.0);
	let frequencies = (10.0 .. 5500.0);
	let fs = image_to_frequencies(&image, frequencies, (0.0 .. max_time), legend );
	
	{
		use std::io::Write;
		let mut of = ::std::fs::File::create("amps.data").unwrap();
		for &(f,ref a) in &fs {
			for s in a {
				write!(of, "{} ", s);
			}
			write!(of, "\n");
		}
	}
	
	//let samplerate = 44.1e3;
	let samplerate = 96e3;
	let mut audio = frequencies_to_waveform(samplerate, max_time, &fs);
	
	normalise(&mut audio);
	
	{
		use byteorder::{WriteBytesExt, LittleEndian};
		let mut of = ::std::fs::File::create("out.pcm").unwrap();
		//let mut of2 = ::std::fs::File::create("out.txt").unwrap();
		for sample in &audio {
			use ::std::io::Write;
			//writeln!(&mut of2, "{}", sample);
			assert!(sample.abs() <= 1.0);
			of.write_i16::<LittleEndian>( (sample * 0x7FFF as f64) as i16 );
		}
	}
}

struct Image {
	data: Vec<u8>,
	width: usize,
}
impl Image {
	fn height(&self) -> usize { self.data.len() / self.width }
	fn width(&self) -> usize { self.width }
	fn rows(&self) -> ::std::slice::Chunks<u8> {
		self.data.chunks(self.width)
	}
}

fn load_image() -> Image
{
	use image::GenericImage;
	use image::Pixel;
	
	// TODO: Replace piston image with something lighter
	//let pi = image::load(::std::fs::File::open("../GroupProjectData/CleanSignal_Spectrogram.jpg").unwrap(), image::ImageFormat::JPEG).unwrap();
	let pi = image::load(::std::fs::File::open("input.png").unwrap(), image::ImageFormat::PNG).unwrap();
	error!("Image in memory");
	let mut ret = Image {
		data: ::std::iter::repeat(0).take( (pi.width()*pi.height()) as usize ).collect(),
		width: pi.width() as usize,
		};
	error!("Loading pixel data");
	for px in pi.pixels()
	{
		ret.data[ (px.0 + px.1 * pi.width()) as usize ] = px.2.to_luma().data[0];
	}
	ret
}

fn pixel_value_to_amplitude(colour_db_range: Range<f32>, value: u8) -> f64
{
	let db = colour_db_range.start + (value as f32 / 255.0) * (colour_db_range.end - colour_db_range.start);
	if true {
		(10.0_f64).powf(db as f64 / 10.0).sqrt()
	}
	else {
		db as f64
	}
}

fn image_to_frequencies(image: &Image, freqrange: Range<f64>, timerange: Range<f32>, col_range: Range<f32>) -> Vec<(f64, Vec<f64>)>
{
	let fstep = (freqrange.end - freqrange.start) / image.height() as f64;
	let mut ret = Vec::new();
	for (i,row) in image.rows().enumerate()
	{
		let f = freqrange.start + (image.height()-1 - i) as f64 * fstep;
		trace!("{:3}: f={:.0}Hz", i, f);
		
		let pxvals = row/*.pixels()*/.iter().cloned();
		//let pxvals = ::std::iter::repeat( row[100] ).take( row.len() );
		let vals = pxvals.map(|p| pixel_value_to_amplitude(col_range.clone(), p)).collect();
		ret.push( (f, vals) );
		//break;
	}
	debug!("{} rows", ret.len());
	ret
}
fn normalise(samples: &mut [f64])
{
	use num::traits::Float;
	//let max_a = samples.iter().map(|v| v.abs()).max().unwrap();
	let max_a = samples.iter().map(|v| v.abs()).fold(-1000.0, |a, b| a.max(b));
	debug!("Peak amplitude {}", max_a);
	let scale = 0.9 / max_a;
	for s in samples { *s *= scale; }
}

// block_timeslice = Length of a block in seconds
// samples_per_block = Number of samples to generate per block
fn frequencies_to_waveform(sample_rate: f64, total_time: f32, freqs: &[(f64, Vec<f64>)]) -> Vec<f64>
{
	let block_count = freqs[0].1.len();
	let samples_per_block = ((sample_rate * total_time as f64) / block_count as f64) as usize;
	// Interpolate 5% on each end of each block
	let interpolation_count = samples_per_block / 10;
	let t_step = 1.0 / sample_rate;
	debug!("{} blocks, {} spb, {} s per sample", block_count, samples_per_block, t_step);
	let mut output: Vec<f64> = ::std::iter::repeat(0.0).take(samples_per_block * block_count).collect();
	// TODO: Interpolate between frequencies
	for (freq_i, &(f, ref amps)) in freqs.iter().enumerate()
	{
		trace!("{:.0}Hz: {} samples", f, amps.len());
		let omega = ::std::f64::consts::PI * 2.0 * f as f64;
		let mut t = (freq_i as f64 * t_step * 10.0).powi(2);
		let t_step = t_step * omega;
		trace!("- t={},t_step={},omega={}", t, t_step, omega);
		
		for (block_idx,(block_amp,blk_out)) in amps.iter().zip(output.chunks_mut(samples_per_block)).enumerate()
		{
			// Check for interpolation on each side
			let start = if interpolation_count > 0 && block_idx > 0 {
					interpolation_count
				}
				else {
					0
				};
			let end = samples_per_block - if interpolation_count > 0 && block_idx + 1 < amps.len() {
					interpolation_count
				}
				else {
					0
				} - start;

			// Generate data
			// - Head interpolation
			if start != 0 {
				let prev_amp = (amps[block_idx-1] + block_amp)/2.0;
				// Work forwards into the block: j=0 :: a=prev
				for i in (0 .. interpolation_count) {
					let p = i as f64 / (interpolation_count-1) as f64;
					let a = block_amp * p + prev_amp * (1.0 - p);
					blk_out[i] += a * f64::sin(t);
					t += t_step;
				}
			}
			// - The body (constant amplitude)
			for i in (start .. end)
			{
				blk_out[i] += block_amp * f64::sin( t );
				t += t_step;
			}
			// - Tail interpolation
			if end != samples_per_block - start {
				let next_amp = (amps[block_idx+1] + block_amp)/2.0;
				// Work backwards into the block: j=0 :: a=next_amp
				for j in (0 .. interpolation_count) {
					let p = j as f64 / (interpolation_count-1) as f64;
					let a = block_amp * p + next_amp * (1.0 - p);
					blk_out[samples_per_block - j - 1] += a * f64::sin(t);
					t += t_step;
				}
			}
			
			// Move t down into a single cycle again
			// - Speeds up sin, and reduces artifacts
			while t > ::std::f64::consts::PI * 2.0 {
				t -= ::std::f64::consts::PI * 2.0;
			}
		}
	}
	output
}

// vim: ft=rust
