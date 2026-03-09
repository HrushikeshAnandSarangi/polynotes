use std::ffi::{CStr, CString};
pub use std::fmt;
pub use std::write;

use thiserror::Error;

pub mod bindings;

///Errors for the core crate
#[derive(Debug,Error)]
pub enum CoreError{
    #[error("Model path contains interior null bytes")]
    InvalidPath,

    #[error("whisper_context_default_params returned a null")]
    NullParams,

    #[error("Failed to load model from '{path}'")]
    ModelLoadFailed{path:String},

    #[error("whisper_full() returned error code {0}")]
    InferenceFailed(i32),

    #[error("Segment text pointer was null for segment {0}")]
    NullSegmentText(i32),
}


///Transcription options struct and
///its implication block for default options
pub struct TranscribeOptions{
    pub language:String,
    pub n_threads:i32,
    pub translate:bool,
    pub suppress_blank:bool,
}
impl Default for TranscribeOptions{
    fn default() -> Self {
        Self{
            language:"auto".into(),
            n_threads:4,
            translate:false,
            suppress_blank:true,
        }
    }
}

///Struct for each segment
#[derive(Debug,Clone)]
pub struct Segment{
    pub start_ms:i64,
    pub end_ms:i64,
    pub text:String,
}


///Struct context wrapper for the loaded whisper model.
///Whisper.cpp context is not mutated outside of whisper_full so send and sync lifetimes are added
///for implementation.
pub struct WhisperContext{
    ctx: *mut bindings::whisper_context,
}
unsafe impl  Send for WhisperContext{}
unsafe impl Sync for WhisperContext{}


impl WhisperContext {
    pub fn new(model_path:&str)->Result<Self,CoreError>{
        let c_path=CString::new(model_path).map_err(|_|CoreError::InvalidPath)?;
        let cparams=unsafe {
            bindings::whisper_context_default_params()
        };

        let ctx=unsafe {
            bindings::whisper_init_from_file_with_params(
                c_path.as_ptr(),
                cparams,
            )
        };
        if ctx.is_null(){
            return Err(CoreError::ModelLoadFailed { path: model_path.to_owned() });
        }

        Ok(Self{ctx})
    }

    fn build_params(&self,opts:&TranscribeOptions,)->bindings::whisper_full_params{
        let mut params=unsafe {
            bindings::whisper_full_default_params(bindings::whisper_sampling_strategy_WHISPER_SAMPLING_GREEDY,)
        };

        params.n_threads=if opts.n_threads>0{opts.n_threads}else {
            4
        };
        params.translate=opts.translate;
        params.suppress_blank=opts.suppress_blank;
        params.print_special=false;
        params.print_progress=false;
        params.print_realtime=false;
        params.print_timestamps=false;
        params.no_context=true;
        
        // Tuning parameters to prevent hallucinations and radically speed up inference
        params.no_timestamps=true;
        params.single_segment=true;
        params.temperature_inc=0.0;
        params.entropy_thold=2.8;
        params.logprob_thold=-1.0;
        params.no_speech_thold=0.6;

        if opts.language != "auto"{
            let lang=CString::new(opts.language.as_str()).unwrap();
            params.language=lang.into_raw();
        }
        params

    }

    pub fn transcribe_segments(&self,audio:&[f32],opts:TranscribeOptions)->Result<Vec<Segment>,CoreError>{
        let params=self.build_params(&opts);
        let ret=unsafe {
            bindings::whisper_full(self.ctx,params,audio.as_ptr(),audio.len() as i32)
        };

        if !params.language.is_null() && opts.language != "auto"{
            unsafe {drop(CString::from_raw(params.language as *mut _));}
        }
        if ret!=0{
            return Err(CoreError::InferenceFailed(ret));
        }

        let n=unsafe {
            bindings::whisper_full_n_segments(self.ctx)
        };

        let mut out=Vec::with_capacity(n as usize);

        for i in 0..n{
            let ptr=unsafe {
                bindings::whisper_full_get_segment_text(self.ctx,i)
            };
            if ptr.is_null(){
                return Err(CoreError::NullSegmentText(i));
            }

            let text=unsafe {
                CStr::from_ptr(ptr)
            }.to_string_lossy()
                .trim()
                .to_owned();

            let start_ms=unsafe {
                bindings::whisper_full_get_segment_t0(self.ctx,i)
            };

            let end_ms=unsafe {
                bindings::whisper_full_get_segment_t1(self.ctx,i)
            };

            out.push(Segment{start_ms,end_ms,text});
        }
        Ok(out)

    }

}

impl Drop for WhisperContext{
    fn drop(&mut self){
        if !self.ctx.is_null(){
            unsafe {bindings::whisper_free(self.ctx)}
        }
    }
}

mod tests;
