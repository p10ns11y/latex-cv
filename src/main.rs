use std::process::Command;
use std::fs::read_dir;

use aws_config::meta::region::RegionProviderChain;
use aws_config::BehaviorVersion;
use aws_sdk_s3::{Client, Error as S3Error};
use aws_sdk_s3::primitives::ByteStream;

use tokio::fs::File;
use tokio::io::AsyncReadExt;

use thiserror::Error;

#[derive(Error, Debug)]
enum UploadError {
    #[error("S3 error: {0}")]
    S3Error(#[from] S3Error),
    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),
}

// write a function that uploads generated PDFs to an AWS S3 bucket
async fn upload_to_aws_s3() -> Result<(), UploadError> {
  print!("Uploading PDFs to S3...");
  let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
  let config = aws_config::defaults(BehaviorVersion::latest())
        .region(region_provider)
        .load()
        .await;
  let client = Client::new(&config);


  let paths = read_dir("./pdfs").unwrap();
  for path in paths {
    let path = path.map_err(UploadError::IOError)?.path();
    if path.extension().unwrap() == "pdf" {
        let mut file = File::open(&path).await.map_err(UploadError::IOError)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).await.map_err(UploadError::IOError)?;
        let byte_stream = ByteStream::from(buffer);

        client.put_object()
            .bucket("toileredcvs")
            .key(path.file_name().unwrap().to_str().unwrap())
            .body(byte_stream)
            .send()
            .await
            .map_err(|e| UploadError::S3Error(e.into()))?;
      }
  }

  Ok(())
}

#[tokio::main]
async fn main() -> Result<(), UploadError> {
  println!("Generating PDFs...");
  let output = Command::new("sh")
      .arg("-c")
      .arg("find . -maxdepth 1 -name 'cv*.tex' -exec pdflatex {} \\;")
      .output()
      .expect("failed to execute process");

  println!("{}", output.status);
  // println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
  println!("stderr: {}", String::from_utf8_lossy(&output.stderr));

  // fix: move the pdf to a folder called 'pdfs' and create the output folder if it does not exist
  let output = Command::new("sh")
      .arg("-c")
      .arg("mkdir -p pdfs && mv cv*.pdf pdfs")
      .output()
      .expect("failed to execute process");

  println!("{}",output.status);
  // println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
  println!("stderr: {}", String::from_utf8_lossy(&output.stderr));
  println!("PDFs generated successfully!");

  upload_to_aws_s3().await?;

  Ok(())
}
