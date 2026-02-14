use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JobStatus {
    Queued,
    Processing,
    MediaExtraction,
    Transcribing,
    Embedding,
    Completed,
    Failed,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Queued => "QUEUED",
            JobStatus::Processing => "PROCESSING",
            JobStatus::MediaExtraction => "MEDIA_EXTRACTION",
            JobStatus::Transcribing => "TRANSCRIBING",
            JobStatus::Embedding => "EMBEDDING",
            JobStatus::Completed => "COMPLETED",
            JobStatus::Failed => "FAILED",
        }
    }
}

impl FromStr for JobStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "QUEUED" => Ok(JobStatus::Queued),
            "PROCESSING" => Ok(JobStatus::Processing),
            "MEDIA_EXTRACTION" => Ok(JobStatus::MediaExtraction),
            "TRANSCRIBING" => Ok(JobStatus::Transcribing),
            "EMBEDDING" => Ok(JobStatus::Embedding),
            "COMPLETED" => Ok(JobStatus::Completed),
            "FAILED" => Ok(JobStatus::Failed),
            _ => Err(format!("Invalid job status: {}", s)),
        }
    }
}

impl fmt::Display for JobStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
