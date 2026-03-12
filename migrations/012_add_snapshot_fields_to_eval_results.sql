ALTER TABLE eval_results
    ADD COLUMN IF NOT EXISTS question         TEXT,
    ADD COLUMN IF NOT EXISTS generated_answer TEXT,
    ADD COLUMN IF NOT EXISTS eval_description TEXT;
