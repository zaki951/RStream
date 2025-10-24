use anyhow::Result;
use hound;
use std::sync::Arc;
use streamapp::client::client_manager;
use streamapp::server::server_manager;

const ADDRESS: &str = "localhost";
const PORT: u16 = 8080;
const PATH_INPUT: &str = "/tmp/test_input.wav";
const PATH_OUTPUT: &str = "/tmp/test_output.wav";

pub fn compare_wav_samples(file1: &str, file2: &str) -> bool {
    let reader1 = hound::WavReader::open(file1).expect("Cannot open first WAV file");
    let reader2 = hound::WavReader::open(file2).expect("Cannot open second WAV file");

    let spec1 = reader1.spec();
    let spec2 = reader2.spec();

    if spec1.channels != spec2.channels {
        eprintln!(
            "Number of channels differ: {} != {}",
            spec1.channels, spec2.channels
        );
        return false;
    }
    if spec1.sample_rate != spec2.sample_rate {
        eprintln!(
            "Sample rate differ: {} != {}",
            spec1.sample_rate, spec2.sample_rate
        );
        return false;
    }
    if spec1.bits_per_sample != spec2.bits_per_sample {
        eprintln!(
            "Bits per sample differ: {} != {}",
            spec1.bits_per_sample, spec2.bits_per_sample
        );
        return false;
    }
    if spec1.sample_format != spec2.sample_format {
        eprintln!(
            "Sample format differ: {:?} != {:?}",
            spec1.sample_format, spec2.sample_format
        );
        return false;
    }

    match spec1.sample_format {
        hound::SampleFormat::Int => {
            if spec1.bits_per_sample == 16 {
                let samples1 = reader1.into_samples::<i16>();
                let samples2 = reader2.into_samples::<i16>();
                return samples1
                    .zip(samples2)
                    .all(|(a, b)| a.unwrap() == b.unwrap());
            } else if spec1.bits_per_sample == 32 {
                let samples1 = reader1.into_samples::<i32>();
                let samples2 = reader2.into_samples::<i32>();
                return samples1
                    .zip(samples2)
                    .all(|(a, b)| a.unwrap() == b.unwrap());
            } else {
                panic!("Unsupported integer bit depth: {}", spec1.bits_per_sample);
            }
        }
        hound::SampleFormat::Float => {
            let samples1 = reader1.into_samples::<f32>();
            let samples2 = reader2.into_samples::<f32>();
            return samples1
                .zip(samples2)
                .all(|(a, b)| (a.unwrap() - b.unwrap()).abs() < 1e-5);
        }
    }
}

async fn client_task() -> Result<()> {
    let mut handler = client_manager::ClientInterface::connect(ADDRESS.to_string(), PORT)
        .await
        .expect("Failed to connect to server");
    handler
        .add_capability(client_manager::Capabilities::SaveToFile(
            PATH_OUTPUT.to_string(),
        ))
        .start_playing()
        .await
}
#[tokio::test]
async fn test_audio_streaming() -> Result<()> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);

    tokio::spawn(async move {
        let server = Arc::new(
            server_manager::Server::new(ADDRESS.to_string(), PORT, PATH_INPUT.to_string()).await,
        );
        tx.send(()).await.unwrap();
        server.run().await;
    });

    rx.recv().await.unwrap();

    client_task().await?;

    Ok(())
}
