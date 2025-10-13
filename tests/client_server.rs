use anyhow::Result;
use hound;
use std::time::Duration;
use streamapp::client::client_manager;
use streamapp::server::server_manager;

const ADDRESS: &str = "localhost";
const PORT: u16 = 8080;
const PATH_INPUT: &str = "/tmp/test_input.wav";
const PATH_OUTPUT: &str = "/tmp/test_output.wav";

async fn server_task() -> Result<()> {
    let server: server_manager::Server =
        server_manager::Server::new(ADDRESS.to_string(), PORT, PATH_INPUT.to_string());
    server.run().await;

    Ok(())
}

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
    let client = client_manager::ClientSocket {
        address: ADDRESS.to_string(),
        port: PORT,
    };

    let mut handler = client.connect().await.expect("Failed to connect to server");

    handler
        .add_capability(client_manager::Capabilities::SaveToFile(
            PATH_OUTPUT.to_string(),
        ))
        .add_capability(client_manager::Capabilities::RealTimePlayback)
        .start_playing()
        .await
}

#[tokio::test]
async fn test_client_server_file_transfert() {
    assert!(std::path::Path::new(&PATH_INPUT).is_file());
    let _ = tokio::spawn(async { server_task().await });

    tokio::time::sleep(Duration::from_millis(1000)).await;

    client_task().await.unwrap();

    assert!(
        compare_wav_samples(PATH_INPUT, PATH_OUTPUT),
        "Files differ!"
    );
}
