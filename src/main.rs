extern crate nix;
extern crate regex;
extern crate time;

use time::Time;
use nix::sys::mman::{*, ProtFlags, MapFlags};
use regex::Regex;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::os::unix::io::AsRawFd; 
use std::{thread, time as stdtime};


#[derive(Debug)]
struct ProfilingError {
    details: String,
}

impl ProfilingError {
    fn new(msg: &str) -> ProfilingError {
        ProfilingError {
            details: msg.to_string(),
        }
    }
}

impl fmt::Display for ProfilingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

impl Error for ProfilingError {
    fn description(&self) -> &str {
        &self.details
    }
}

struct MMDC {
    mdctl: u32,
    mdpdc: u32,
    mdotc: u32,
    mdcfg0: u32,
    mdcfg1: u32,
    mdcfg2: u32,
    mdmisc: u32,
    mdscr: u32,
    mdref: u32,
    mdwcc: u32,
    mdrcc: u32,
    mdrwd: u32,
    mdor: u32,
    mdmrr: u32,
    mdcfg3lp: u32,
    mdmr4: u32,
    mdasp: u32,

    adopt_base_offset_fill: [u32; 239],
    maarcr: u32,
    mapsr: u32,
    maexidr0: u32,
    maexidr1: u32,
    madpcr0: u32,
    madpcr1: u32,
    madpsr0: u32,
    madpsr1: u32,
    madpsr2: u32,
    madpsr3: u32,
    madpsr4: u32,
    madpsr5: u32,
    masbs0: u32,
    masbs1: u32,
    ma_reserved1: u32,
    ma_reserved2: u32,
    magenp: u32,

    phy_base_offset_fill: [u32; 239],
    mpzqhwctrl: u32,
    mpzqswctrl: u32,
    mpwlgcr: u32,
    mpwldectrl0: u32,
    mpwldectrl1: u32,
    mpwldlst: u32,
    mpodtctrl: u32,
    mpredqby0dl: u32,
    mpredqby1dl: u32,
    mpredqby2dl: u32,
    mpredqby3dl: u32,
    mpwrdqby0dl: u32,
    mpwrdqby1dl: u32,
    mpwrdqby2dl: u32,
    mpwrdqby3dl: u32,
    mpdgctrl0: u32,
    mpdgctrl1: u32,
    mpdgdlst: u32,
    mprddlctl: u32,
    mprddlst: u32,
    mpwrdlctl: u32,
    mpwrdlst: u32,
    mpsdctrl: u32,
    mpzqlp2ctl: u32,
    mprddlhwctl: u32,
    mpwrdlhwctl: u32,
    mprddlhwst0: u32,
    mprddlhwst1: u32,
    mpwrdlhwst0: u32,
    mpwrdlhwst1: u32,
    mpwlhwerr: u32,
    mpdghwst0: u32,
    mpdghwst1: u32,
    mpdghwst2: u32,
    mpdghwst3: u32,
    mppdcmpr1: u32,
    mppdcmpr2: u32,
    mpswdar: u32,
    mpswdrdr0: u32,
    mpswdrdr1: u32,
    mpswdrdr2: u32,
    mpswdrdr3: u32,
    mpswdrdr4: u32,
    mpswdrdr5: u32,
    mpswdrdr6: u32,
    mpswdrdr7: u32,
    mpmur: u32,
    mpwrcadl: u32,
    mpdccr: u32,
    mpbc: u32,
}

#[derive(Default)]
struct MMDCProfileResult {
    total_cycles: u32,
    busy_cycles: u32,
    read_accesses: u32,
    write_accesses: u32,
    read_bytes: u32,
    write_bytes: u32,
    data_load: u32,
    utilization: u32,
    access_utilization: u32,
    avg_write_burstsize: u32,
    avg_read_burstsize: u32,
}

enum MMDCResultType {
    Full,
    Utilization,
}

static AXI_IPU1: u32 = 0x3FE70004;
static AXI_IPU2_6Q: u32 = 0x3FE70005;
static AXI_GPU3D_6DL: u32 = 0x003F0002;
static AXI_GPU3D_6Q: u32 = 0x003E0002;
static AXI_GPU2D2_6DL: u32 = 0x003F0003;
static AXI_GPU2D1_6DL: u32 = 0x003F000A;
static AXI_GPU2D_6Q: u32 = 0x003E000B;
static AXI_GPU2D_6SL: u32 = 0x0017000F;
static AXI_VPU_6DL: u32 = 0x003F000B;
static AXI_VPU_6Q: u32 = 0x003F0013;
static AXI_OPENVG_6Q: u32 = 0x003F0022;
static AXI_OPENVG_6SL: u32 = 0x001F0017;
static AXI_ARM: u32 = 0x00060000;
static AXI_PCIE: u32 = 0x303F001B;
static AXI_SATA: u32 = 0x3FFF00E3;
static AXI_DEFAULT: u32 = 0x00000000;

static MMDC_P0_IPS_BASE_ADDR : i64 = 0x021B0000;
static MMDC_P1_IPS_BASE_ADDR : i64 = 0x021B4000;

fn get_system_revision() -> Result<u32, ProfilingError> {
    let mut f = match File::open("/home/hggw/priv/r-mmdc/src/cpu_info.dummy") {
        //TODO: /proc/cpuinfo
        Ok(file) => file,
        Err(_) => return Err(ProfilingError::new("Error opening /proc/cpuinfo")),
    };

    let mut buffer = [0_u8; 2048];

    match f.read(&mut buffer) {
        Ok(rsize) => {
            println!("/proc/cpuinfo read size: {}", rsize);
            if rsize == 0 || rsize == 2048 {
                return Err(ProfilingError::new(
                    "Error reading cpu info, no bytes read or buffer full",
                ));
            }
            rsize
        }
        Err(_) => return Err(ProfilingError::new("Error reading cpu info")),
    };



    let read_string = String::from_utf8_lossy(&buffer);
    //find Revision: <something in string>
    let re = Regex::new(r"Revision\s*:\s*([a-fA-F0-9]+)").unwrap(); //lotso unwraping, it's like christmas
    let revision_string = &(re.captures(&read_string).unwrap())[1];
    let revision = u32::from_str_radix(revision_string, 16).unwrap();
    println!("CPU Revision is {:X?}", revision);

    if revision == 0u32 {
        let mut sbuffer = [0_u8; 2048]; // just to be sure, prevent strange behaviour by buffer reusage
        let mut soc_file = match File::open("/home/hggw/priv/r-mmdc/src/soc_id.dummy") {
            //TODO: /sys/devices/soc0/soc_id
            Ok(file) => file,
            Err(_) => {
                return Err(ProfilingError::new(
                    "Error opening /sys/devices/soc0/soc_id",
                ))
            }
        };

        match soc_file.read(&mut sbuffer) {
            Ok(rsize) => {
                println!("/sys/devices/soc0/soc_id read size: {}", rsize);
                if rsize == 0 || rsize == 2048 {
                    return Err(ProfilingError::new(
                        "Error reading soc id, no bytes read or buffer full",
                    ));
                }
                
            }
            Err(_) => return Err(ProfilingError::new("Error reading cpu info")),
        };
        let soc_id :String= String::from_utf8_lossy(&sbuffer).to_string();
        println!("Read soc id {}", soc_id);
        return if soc_id.starts_with("i.MX6Q") {
          Ok(0x63000u32)
        } else if  soc_id.starts_with("i.MX6DL") {
           Ok(0x61000u32)
        }else if soc_id.starts_with("i.MX6SL") {
            Ok(0x60000u32)
        }else {
            Err(ProfilingError::new("Unknown soc id2"))
        }
    }
    Err(ProfilingError::new("Unknown soc id"))
}

fn print_profiling_results(profiling_result: &MMDCProfileResult, time: u64) {
    //why the hell is time an int32?
    println!("MMDC new Profiling results:");
    println!("***********************");
    println!("***********************\n");
    println!("Measure time: {}ms", time);
    println!("Total cycles count: {}", profiling_result.total_cycles);
    println!("Busy cycles count: {}", profiling_result.busy_cycles);
    println!("Read accesses count: {}", profiling_result.read_accesses);
    println!("Write accesses count: {}", profiling_result.write_accesses);
    println!("Read bytes count: {}", profiling_result.read_bytes);
    println!("Write bytes count: {}", profiling_result.write_bytes);
    println!(
        "Avg. Read burst size: {}",
        profiling_result.avg_read_burstsize
    );
    println!(
        "Avg. Write burst size: {}",
        profiling_result.avg_write_burstsize
    );

    let avg_read: f32 =
        profiling_result.write_bytes as f32 * 1000_f32 / (1024_f32 * 1024_f32 * time as f32);
    let avg_write: f32 =
        profiling_result.write_bytes as f32 * 1000_f32 / (1024_f32 * 1024_f32 * time as f32);
    let total: f32 = (profiling_result.write_bytes as f32 + profiling_result.read_bytes as f32)
        * 1000_f32
        / (1024_f32 * 1024_f32 * time as f32);
    println!(
        "Read: {:.2} MB/s /  Write: {:.2} MB/s  Total: {:.2} MB/s",
        avg_read, avg_write, total
    );

    println!("");
}

fn get_mmdc_profiling_results(mmdc: &MMDC) -> MMDCProfileResult {
    let mut result = MMDCProfileResult::default();

    result.total_cycles = mmdc.madpsr0;
    result.busy_cycles = mmdc.madpsr1;
    result.read_accesses = mmdc.madpsr2;
    result.write_accesses = mmdc.madpsr3;
    result.read_bytes = mmdc.madpsr4;
    result.write_bytes = mmdc.madpsr5;

    if result.read_bytes != 0 || result.write_bytes != 0 {
        result.utilization = ((result.read_bytes as f32 + result.write_bytes as f32)
            / (result.busy_cycles as f32 * 16_f32)
            * 100_f32) as u32;
        result.data_load =
            (result.busy_cycles as f32 / result.total_cycles as f32 * 100_f32) as u32;
        result.access_utilization = ((result.read_bytes as f32 + result.write_bytes as f32)
            / (result.read_accesses as f32 + result.write_accesses as f32))
            as u32;
    }

    if mmdc.madpsr3 > 0 {
        result.avg_write_burstsize = mmdc.madpsr5 / mmdc.madpsr2;
    } //no else branch needed, default 0

    if mmdc.madpsr2 > 0 {
        result.avg_read_burstsize = mmdc.madpsr4 / mmdc.madpsr2;
    } //no else branch needed, default 0

    result
}

fn get_tick_count() -> u64 {
   let t = time::Time::now();
   ((t.second() as u32 *1000u32 ) as u64 / (t.microsecond() as u32 /1000 as u32) as u64) as u64
}

fn clear_mmdc(mmdc: &mut MMDC){
    mmdc.madpcr0 = 0xA; // Reset counters and clear Overflow bit
    unsafe {
        msync(&mut mmdc.madpcr0 as *mut _ as *mut _, 4, MsFlags::MS_SYNC).unwrap(); 
    }
}

fn start_mmdc_profiling(mmdc: &mut MMDC){
    unsafe {  
         mmdc.madpcr0 = 0xA;		// Reset counters and clear Overflow bit
        msync(&mut mmdc.madpcr0 as *mut _ as *mut _,4, MsFlags::MS_SYNC).unwrap();
    
        mmdc.madpcr0 = 0x1;		// Enable counters
        msync(&mut mmdc.madpcr0 as *mut _ as *mut _,4,MsFlags::MS_SYNC).unwrap();
    }
}

fn load_mmdc_results(mmdc: &mut MMDC){
    mmdc.madpcr0 |= 0x4; //sets the PRF_FRZ bit to 1 in order to load the results into the registers
    unsafe {
        msync(&mut mmdc.madpcr0 as *mut _ as *mut _,4,MsFlags::MS_SYNC).unwrap();
    }
}

fn stop_mmdc_profiling(mmdc: &mut MMDC)
{
    mmdc.madpcr0 = 0x0;		// Disable counters
    unsafe {
        msync(&mut mmdc.madpcr0 as *mut _ as *mut _,4,MsFlags::MS_SYNC).unwrap();
    }
}

fn main() {
   
    let mmdc : &mut MMDC;
     unsafe{
        let protflag: ProtFlags = ProtFlags::from_bits(0x03).expect("invalid protflags");
    
        let fd = match File::open("/dev/mem") {
            Err(e) => panic!("couldn't open /dev/mem: {}", e),
            Ok(file) => file,
        };
        match mmap(std::ptr::null_mut(), 0x4000, protflag, MapFlags::MAP_SHARED, fd.as_raw_fd() , MMDC_P0_IPS_BASE_ADDR){
            Ok(p) => mmdc = &mut *(p as *mut MMDC),
            Err(_) => panic!("Error mapping memory")
        };
    };
    println!("Succesfully mapped memory");

    //let allow_parameters = match get_system_revision() {
    //    Ok(_) => true,
    //    Err(_) => false,
    //}; UNUSED FOR NOW


    clear_mmdc(mmdc);
    let start_time = get_tick_count();
    start_mmdc_profiling(mmdc);
    thread::sleep(stdtime::Duration::from_secs(1));
    load_mmdc_results(mmdc);
    let results = get_mmdc_profiling_results(mmdc);
    print_profiling_results(&results, get_tick_count() - start_time);
    stop_mmdc_profiling(mmdc);
}
