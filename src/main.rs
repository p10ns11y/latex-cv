use std::fs::read_dir;
use std::process::Command;

use aws_config::meta::region::RegionProviderChain;
use aws_config::BehaviorVersion;
use aws_sdk_s3::operation::head_bucket::HeadBucketError;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::{
    error::SdkError,
    types::{CreateBucketConfiguration, PublicAccessBlockConfiguration, VersioningConfiguration},
    Client, Error as S3Error,
};

use tokio::fs::File;
use tokio::io::AsyncReadExt;

use thiserror::Error;

static BUCKET_NAME: &str = "peramsathyam";

#[derive(Error, Debug)]
enum UploadError {
    #[error("S3 error: {0}")]
    S3Error(#[from] S3Error),
    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),
}

async fn upload_to_aws_s3() -> Result<(), UploadError> {
    print!("Uploading CVs to S3...");
    let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
    let config = aws_config::defaults(BehaviorVersion::latest())
        .region(region_provider)
        .load()
        .await;
    let client = Client::new(&config);

    create_bucket_if_not_exists(&client).await?;
    set_bucket_policy(&client).await?;
    enable_bucket_versioning(&client).await?;

    let paths = read_dir("./pdfs").unwrap();
    for path in paths {
        let path = path.map_err(UploadError::IOError)?.path();
        if path.extension().unwrap() == "pdf" {
            let mut file = File::open(&path).await.map_err(UploadError::IOError)?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)
                .await
                .map_err(UploadError::IOError)?;
            let byte_stream = ByteStream::from(buffer);

            client
                .put_object()
                .bucket(BUCKET_NAME)
                .key(format!(
                    "cvs/{}",
                    path.file_name().unwrap().to_str().unwrap()
                ))
                .body(byte_stream)
                .content_disposition("inline")
                .content_type("application/pdf")
                .send()
                .await
                .map_err(|e| UploadError::S3Error(e.into()))?;
        }
    }

    print!("CVs Uploaded to S3");

    Ok(())
}

async fn create_bucket_if_not_exists(client: &Client) -> Result<(), UploadError> {
    let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");

    match client.head_bucket().bucket(BUCKET_NAME).send().await {
        Ok(_) => {
            println!("Bucket '{}' already exists.", BUCKET_NAME);
        }
        Err(SdkError::ServiceError(ref e)) if matches!(e.err(), HeadBucketError::NotFound(_)) => {
            client
                .create_bucket()
                .bucket(BUCKET_NAME)
                .create_bucket_configuration(
                    CreateBucketConfiguration::builder()
                        .location_constraint(
                            region_provider.region().await.unwrap().as_ref().into(),
                        )
                        .build(),
                )
                .send()
                .await
                .map_err(|e| UploadError::S3Error(e.into()))?;

            // Disable Block Public Access settings
            client
                .put_public_access_block()
                .bucket(BUCKET_NAME)
                .public_access_block_configuration(
                    PublicAccessBlockConfiguration::builder()
                        .block_public_acls(true) // Block public ACLs
                        .ignore_public_acls(true) //  Ignore public ACLs
                        .block_public_policy(false) // Allow public bucket policies
                        .restrict_public_buckets(false) // Do not restrict public buckets
                        .build(),
                )
                .send()
                .await
                .map_err(|e| UploadError::S3Error(e.into()))?;

            println!(
                "Bucket '{}' created with public access settings.",
                BUCKET_NAME
            );
        }
        Err(e) => {
            return Err(UploadError::S3Error(e.into()));
        }
    }

    Ok(())
}

async fn set_bucket_policy(client: &Client) -> Result<(), UploadError> {
    client
        .put_bucket_policy()
        .bucket(BUCKET_NAME)
        .policy(&format!(
            r#"{{
            "Version": "2012-10-17",
            "Statement": [
                {{
                    "Effect": "Allow",
                    "Principal": "*",
                    "Action": "s3:GetObject",
                    "Resource": "arn:aws:s3:::{}/*"
                }}
            ]
        }}"#,
            BUCKET_NAME
        ))
        .send()
        .await
        .map_err(|e| UploadError::S3Error(e.into()))?;

    Ok(())
}

async fn enable_bucket_versioning(client: &Client) -> Result<(), UploadError> {
    client
        .put_bucket_versioning()
        .bucket(BUCKET_NAME)
        .versioning_configuration(
            VersioningConfiguration::builder()
                .status(aws_sdk_s3::types::BucketVersioningStatus::Enabled)
                .build(),
        )
        .send()
        .await
        .map_err(|e| UploadError::S3Error(e.into()))?;

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

    let output = Command::new("sh")
        .arg("-c")
        .arg("mkdir -p pdfs && mv cv*.pdf pdfs")
        .output()
        .expect("failed to execute process");

    println!("{}", output.status);
    // println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
    println!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    println!("PDFs generated successfully!");

    upload_to_aws_s3().await?;

    Ok(())
}
