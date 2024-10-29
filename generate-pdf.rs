use std::process::Command;

fn main() {
    println!("Generating PDFs...");
    let output = Command::new("sh")
        .arg("-c")
        .arg("find . -maxdepth 1 \\( -name 'cv*.tex' -o -name 'cl*.tex' \\) -exec pdflatex {} \\;")
        .output()
        .expect("failed to execute process");

    println!("{}", output.status);
    // println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
    println!("stderr: {}", String::from_utf8_lossy(&output.stderr));

    // fix: move the pdf to a folder called 'pdfs' and create the output folder if it does not exist
    let output = Command::new("sh")
        .arg("-c")
        .arg("mkdir -p localpdfs && mv cv*.pdf cl*.pdf localpdfs/")
        .output()
        .expect("failed to execute process");

    println!("{}", output.status);
    // println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
    println!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    println!("PDFs generated successfully!");
}
