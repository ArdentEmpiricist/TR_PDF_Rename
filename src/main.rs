use std::{
    env::args,
    ffi::OsStr,
    fs::{self, read_dir},
    path::{Path, PathBuf},
};

fn main() -> std::io::Result<()> {
    //get path or filename from args
    let path = PathBuf::from(args().nth(1).expect("no file or directory provided"));

    //Alternatively filename can be specified here. Add // to line above and remove at line below + enter path
    //let path = PathBuf::from(r"filename");

    //print path/file provided to stdout
    println!("path or file: {:?}", path);

    //check is path is file or directory
    if path.is_file() && path.extension().and_then(OsStr::to_str) == Some("pdf") {
        rename(&path)?;
    } else if path.is_dir() {
        //println!("Is dir: {:?}",&path);
        for entry in read_dir(&path).expect("error parsing 'entry in read_dir(&path)'") {
            let entry = entry.expect("error unwrapping entry");
            let file_path = entry.path();
            //check if path is file, is a pdf file and if the filename does not start with "20" (as this would indicate it already got renamed)
            if file_path.is_file()
                && file_path.extension().and_then(OsStr::to_str) == Some("pdf")
                && !entry.file_name().to_str().unwrap().starts_with("20")
            {
                let name = rename(&file_path)?;
                println!(
                    "Renamed {:?} to {:?}",
                    entry.file_name(),
                    name.file_name().unwrap()
                );
            } else if file_path.is_file()
                && file_path.extension().and_then(OsStr::to_str) == Some("pdf")
                && entry.file_name().to_str().unwrap().starts_with("20")
            {
                println!(
                    "File {:?} ignored as it seems to have been renamed already.",
                    entry.file_name()
                );
            }
        }
    }

    Ok(())
}

pub fn rename(path: &Path) -> std::io::Result<PathBuf> {
    //prepare the new path to rename the file
    let mut new_path = PathBuf::new();

    //add parent path to new path and clone for further use
    new_path.push(path.parent().unwrap());

    let mut unique_path = new_path.clone();

    //read pdf file
    let bytes = std::fs::read(path).unwrap();
    let out = pdf_extract::extract_text_from_mem(&bytes).unwrap();

    //println!("Read: {}", out);

    //find date of transaction and create string yyyy_mm_dd_
    let position_date: usize = out.clone().find("DATUM").unwrap() + 5;

    let mut date: String = String::new();

    for i in 0..11 {
        date.push(out.clone().chars().nth(position_date + i).unwrap())
    }

    //println!("{:?}", date);

    //trim whitespaces and split date
    let vec_date: Vec<&str> = date.trim().split('.').collect();

    //organise date to yyyy_mm_dd
    let mut date_ordertype_name: String = String::new();
    date_ordertype_name.push_str(vec_date[2]);
    date_ordertype_name.push('_');
    date_ordertype_name.push_str(vec_date[1]);
    date_ordertype_name.push('_');
    date_ordertype_name.push_str(vec_date[0]);
    date_ordertype_name.push('_');

    //println!("date: {:?}", date_ordertype_name);

    //find order type and name
    let mut line_name: usize = 9999;

    let mut name = String::new();

    let mut order_type: String = String::new();

    //take inbto account the different formatting
    if out.contains("DIVIDENDE") {
        order_type = "Dividende".to_string();
        for (i, line) in out.lines().enumerate() {
            if line.starts_with("POSITION") {
                line_name = i + 2;
                //println!("Line with POSITION: {:?},{:?}, {:?}", line, i, line_name);
            } else if i == line_name {
                //println!("Line Name: {:?}", line);
                name = line.to_string();
                break;
            }
        }
    } else if out.contains("SAVEBACK") {
        order_type = "Wertpapierabrechnung_Saveback".to_string();
        for (i, line) in out.lines().enumerate() {
            if line.starts_with("POSITION") {
                line_name = i + 2;
                //println!("Line with POSITION: {:?},{:?}, {:?}", line, i, line_name);
            } else if i == line_name {
                //println!("Line Name: {:?}", line);
                name = line.to_string();
            }
        }
    } else if out.contains("SPARPLAN") {
        order_type = "Wertpapierabrechnung_Sparplan".to_string();
        for (i, line) in out.lines().enumerate() {
            if line.starts_with("POSITION") {
                line_name = i + 2;
                //println!("Line with POSITION: {:?},{:?}, {:?}", line, i, line_name);
            } else if i == line_name {
                //println!("Line Name: {:?}", line);
                name = line.to_string();
            }
        }
    } else if out.contains("WERTPAPIERABRECHNUNG") {
        order_type = "Wertpapierabrechnung".to_string();
        for (i, line) in out.lines().enumerate() {
            if line.starts_with("POSITION") {
                line_name = i + 2;
                //println!("Line with POSITION: {:?},{:?}, {:?}", line, i, line_name);
            } else if i == line_name {
                //println!("Line Name: {:?}", line);
                name = line.to_string();
            }
        }
    } else if out.contains("DEPOTTRANSFER") {
        order_type = "Depottransfer".to_string();
        for line in out.lines() {
            if line.starts_with("1 Depottransfer") {
                name = line
                    .strip_prefix("1 Depottransfer eingegangen ")
                    .unwrap()
                    .to_string();
                //println!("Line with POSITION: {:?},{:?}, {:?}", line, i, line_name);
            }
        }
    };

    //println!("name: {:?}", name);

    //finalize new filename as date_ordertype_name.pdf
    date_ordertype_name.push_str(&order_type);

    date_ordertype_name.push('_');

    date_ordertype_name.push_str(&name);

    new_path.push(date_ordertype_name.clone());

    new_path.set_extension("pdf");

    //rename file

    //check if file exists and add counter to filename to create unique filename
    if new_path.exists() {
        let unique_filename = get_unique_filename(new_path);
        unique_path.push(&unique_filename);
        fs::rename(path, &unique_path)?;
        new_path = unique_path;
    } else {
        fs::rename(path, &new_path)?;
    }
    Ok(new_path)
}

fn get_unique_filename(mut path: PathBuf) -> PathBuf {
    let mut counter = 1;
    let original_path = path.clone();

    while path.exists() {
        let mut new_path = original_path.clone();
        new_path.set_file_name(format!(
            "{}_{}.pdf",
            original_path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(""),
            counter
        ));
        path = new_path;
        counter += 1;
    }

    path
}
