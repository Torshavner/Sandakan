use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub id: ChunkId,
    pub text: String,
    pub document_id: DocumentId,
    pub page: Option<u32>,
    pub offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkId(Uuid);

impl ChunkId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for ChunkId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocumentId(Uuid);

impl DocumentId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for DocumentId {
    fn default() -> Self {
        Self::new()
    }
}

impl Chunk {
    pub fn new(text: String, document_id: DocumentId, page: Option<u32>, offset: usize) -> Self {
        Self {
            id: ChunkId::new(),
            text,
            document_id,
            page,
            offset,
        }
    }
}
