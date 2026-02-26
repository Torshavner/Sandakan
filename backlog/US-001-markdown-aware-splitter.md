### 🚨 Critical Chunking Failures

**1. The "Numbered List" Bug (False Sentence Boundaries)**

* **The Evidence:** Look at the end of Chunk 1:
> `... CIKLUM 5 # How Similarity Search Works \n## The Vector Search Process ### 1.`


* **What Happened:** Your `split_into_sentences` function uses `.` (period) to split sentences. It saw `### 1.` (a markdown numbered list), thought it was the end of a sentence, and cleanly severed the heading from the actual text (`INDEXING Documents → Embedding Model`).
* **The Impact:** If a user asks "What is step 1 of the vector search process?", Chunk 1 has the heading but no answer, and Chunk 2 has the answer but misses the overarching context.

**2. Mid-Data Tearing (Tables and Metrics)**

* **The Evidence:** Look at the boundary between Chunk 3 and Chunk 4. Chunk 3 ends with:
> `- p99 latency: \n - PostgreSQL: 74.60 ms \n - Qdrant: 38.71 ms`
> And Chunk 4 starts immediately with:
> `**Summary:** PostgreSQL with pgvector and pgvectrorscale shows higher latency...`


* **What Happened:** The token limit was reached exactly in the middle of a continuous conceptual block.
* **The Impact:** The LLM receives the raw data in one chunk and the summary in another.

**3. Disjointed Overlaps**

* **The Evidence:** Chunk 2 starts with this overlap:
> `- **1,536 dimensions (for text-embedding-3-small)**\n- **Very close! Distance = 0.02**`
> before transitioning into `### 1. INDEXING`.


* **What Happened:** Because your overlap purely counts tokens backward from the break point, it blindly grabs the last ~50 tokens. In this case, it grabbed the punchline of the previous slide's example, which makes zero sense when attached to the beginning of the "Vector Search Process" slide.

---

### 🛠️ How to Fix This (The Markdown-Aware Approach)

Your current splitter is a pure **Prose Splitter** (Paragraphs → Sentences → Tokens). But your input is **Markdown/Structured Text**.

To fix this, your Semantic Splitter needs to be **Markdown-Aware** before it falls back to sentence splitting.

**1. Split by Markdown Headers First**
Instead of just splitting by `\n\n`, split the document by headers (`#`, `##`, `###`). This guarantees that a section like `# How Similarity Search Works` and its child content are grouped together naturally.

**2. Protect Markdown Blocks**
If you detect a Markdown Table (`|---|---|`) or a code block (```), you should treat the *entire block* as a single sentence/token. Do not allow the sentence splitter to cut a table in half just because it hit the token limit.

**3. Fix the Period Regex**
Update your sentence splitting logic to ignore periods that are immediately preceded by a digit and followed by a space (e.g., `1. `, `2. `) or common abbreviations (`e.g.`, `i.e.`).
In Rust, you can use the `unicode-segmentation` crate's `unicode_sentences()` iterator instead of manually looking for `.`, `!`, `?` — it handles abbreviations and lists much better!

### The Verdict

* **Architecture & Metadata:** 10/10. The logs show your pipeline is robust and memory-efficient.
* **Semantic boundaries:** 4/10. Relying on raw punctuation splitting for Markdown/presentation text is tearing your concepts apart.

