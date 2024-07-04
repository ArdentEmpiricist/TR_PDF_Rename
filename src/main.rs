use std::{
    env::args, ffi::OsStr, fs::{self, read_dir}, path::{Path, PathBuf}
};


fn main() -> std::io::Result<()> {
    //get path or filename from args
    let path = PathBuf::from(args().nth(1).expect("no file or directory provided"));
    
    //Alternatively filename can be specified here. Add // to line above and remove at line below + enter path
    //let path = PathBuf::from(r"filename");

    //print path/file provided to stdout
    println!("path or file: {:?}", path);


//check is path is file or directory
    if path.is_file() && path.extension().and_then(OsStr::to_str) == Some("pdf")  {
            rename(&path)?;
    } else if path.is_dir() {
        //println!("Is dir: {:?}",&path);
        for entry in read_dir(&path).expect("error parsing 'entry in read_dir(&path)'") {
            let entry = entry.expect("error unwrapping entry");
            let file_path = entry.path();
            //check if path is file, is a pdf file and if the filename does not start with "20" (as this would indicate it already got renamed)
            if file_path.is_file() && file_path.extension().and_then(OsStr::to_str) == Some("pdf") && !entry.file_name().to_str().unwrap().starts_with("20") {
                println!("Renamed {:?} to {:?}", entry.file_name(), path.file_name().unwrap());
                    rename(&file_path)?;
            } else if file_path.is_file() && file_path.extension().and_then(OsStr::to_str) == Some("pdf") && entry.file_name().to_str().unwrap().starts_with("20") {
                println!("File {:?} ignored as it seems to have been renamed already.", entry.file_name());
            }
        }
    }

    Ok(())
}

pub fn rename(path: &Path) -> std::io::Result<()> {
    //prepare the new path to rename the file
    let mut new_path = PathBuf::new();

    //add parent path to new path
    new_path.push(path.parent().unwrap());
    
    //read pdf file
    let bytes = std::fs::read(path).unwrap();
    let out = pdf_extract::extract_text_from_mem(&bytes).unwrap();

    //find date of transaction and create string yyyy_mm_dd_
    let position_date: usize = out.clone().find("DATUM").unwrap() + 6;

    let mut date: String = String::new();

    for i in 0..10 {
        date.push(out.clone().chars().nth(position_date + i).unwrap())
    }

    //println!("{:?}", date);

    let vec_date: Vec<&str> = date.split('.').collect();

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

    for (i, line) in out.lines().enumerate() {
        if line.trim_start().starts_with("WERTPAPIERABRECHNUNG") {
            order_type = line.trim_start().to_string();
        } else if line.starts_with("POSITION") {
            line_name = i + 2;
            //println!("Line with POSITION: {:?},{:?}, {:?}", line, i, line_name);
        } else if i == line_name {
            //println!("Line Name: {:?}", line);
            name = line.to_string();
        }
    }

    //println!("name: {:?}", name);


    //finalize new filename as date_ordertype_name.pdf
    date_ordertype_name.push_str(&order_type);

    date_ordertype_name.push('_');

    date_ordertype_name.push_str(&name);

    new_path.push(date_ordertype_name);

    new_path.set_extension("pdf");

    //rename file
    println!("filename: {:?}", new_path);
    fs::rename(path, new_path)?;
    Ok(())
}


