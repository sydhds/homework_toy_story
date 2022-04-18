//! A toy payment rust cli program
//! that you can run with: `cargo run -- resources/sample1_csv > output.csv`

mod accounts;
mod csv_reader;

// std
use std::path::PathBuf;
// third party lib
use log::{debug, error};
// internal
use crate::accounts::{Accounts, TransactionError};
use crate::csv_reader::CsvReader;

/// Our main app error (thanks to thiserror crate)
#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("i/o error: {0}")]
    IO(#[from] std::io::Error),
    #[error("csv error: {0}")]
    Csv(#[from] csv::Error),
    #[error("tx error: {0}")]
    Transaction(#[from] TransactionError),
}

/// run by [main]
fn app_main<P>(csv_path: P) -> Result<(), AppError>
where
    P: Into<PathBuf>,
{
    let csv_path_: PathBuf = csv_path.into();
    // println!("csv_path: {:?}", csv_path_);

    let csv_reader = CsvReader::new(csv_path_)?;

    /*
    for transaction in csv_reader {
        println!("transaction: {:?}", transaction);
    }
    */

    let mut accounts = Accounts::new();

    for transaction_ in csv_reader {
        let transaction = transaction_?;
        debug!("Processing tx: {:?}", transaction);
        accounts.handle_transaction(transaction)?;
    }

    let mut stdout = std::io::stdout();
    accounts.output_as_csv(Some(&mut stdout))?;

    Ok(())
}

/// cli program entry function
fn main() {
    env_logger::init();

    if let Some(csv_path) = std::env::args().nth(1) {
        if let Err(e) = app_main(csv_path) {
            debug!("Error: {:?}", e);
            let return_code = match e {
                AppError::IO(_) => 2,
                AppError::Csv(_) => 3,
                AppError::Transaction(_) => 4,
            };
            std::process::exit(return_code);
        }
    } else {
        error!("Error, Please provide a csv file path, example: cargo run -- foo.csv");
        std::process::exit(1);
    }
}
