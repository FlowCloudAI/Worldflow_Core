ALTER TABLE categories DROP CONSTRAINT IF EXISTS categories_project_id_fkey;
ALTER TABLE categories DROP CONSTRAINT IF EXISTS categories_project_id_parent_id_fkey;
ALTER TABLE tag_schemas DROP CONSTRAINT IF EXISTS tag_schemas_project_id_fkey;
ALTER TABLE entries DROP CONSTRAINT IF EXISTS entries_project_id_fkey;
ALTER TABLE entries DROP CONSTRAINT IF EXISTS entries_category_id_fkey;
ALTER TABLE entry_relations DROP CONSTRAINT IF EXISTS entry_relations_project_id_fkey;
ALTER TABLE entry_relations DROP CONSTRAINT IF EXISTS entry_relations_a_id_fkey;
ALTER TABLE entry_relations DROP CONSTRAINT IF EXISTS entry_relations_b_id_fkey;
ALTER TABLE entry_types DROP CONSTRAINT IF EXISTS entry_types_project_id_fkey;

ALTER TABLE projects
    ALTER COLUMN id TYPE UUID USING id::uuid;

ALTER TABLE categories
    ALTER COLUMN id TYPE UUID USING id::uuid,
    ALTER COLUMN project_id TYPE UUID USING project_id::uuid,
    ALTER COLUMN parent_id TYPE UUID USING parent_id::uuid;

ALTER TABLE tag_schemas
    ALTER COLUMN id TYPE UUID USING id::uuid,
    ALTER COLUMN project_id TYPE UUID USING project_id::uuid;

ALTER TABLE entries
    ALTER COLUMN id TYPE UUID USING id::uuid,
    ALTER COLUMN project_id TYPE UUID USING project_id::uuid,
    ALTER COLUMN category_id TYPE UUID USING category_id::uuid;

ALTER TABLE entry_relations
    ALTER COLUMN id TYPE UUID USING id::uuid,
    ALTER COLUMN project_id TYPE UUID USING project_id::uuid,
    ALTER COLUMN a_id TYPE UUID USING a_id::uuid,
    ALTER COLUMN b_id TYPE UUID USING b_id::uuid;

ALTER TABLE entry_types
    ALTER COLUMN id TYPE UUID USING id::uuid,
    ALTER COLUMN project_id TYPE UUID USING project_id::uuid;

ALTER TABLE categories
    ADD CONSTRAINT categories_project_id_fkey
        FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    ADD CONSTRAINT categories_project_id_parent_id_fkey
        FOREIGN KEY (project_id, parent_id) REFERENCES categories(project_id, id) ON DELETE CASCADE;

ALTER TABLE tag_schemas
    ADD CONSTRAINT tag_schemas_project_id_fkey
        FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE;

ALTER TABLE entries
    ADD CONSTRAINT entries_project_id_fkey
        FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    ADD CONSTRAINT entries_category_id_fkey
        FOREIGN KEY (category_id) REFERENCES categories(id) ON DELETE SET NULL;

ALTER TABLE entry_relations
    ADD CONSTRAINT entry_relations_project_id_fkey
        FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    ADD CONSTRAINT entry_relations_a_id_fkey
        FOREIGN KEY (a_id) REFERENCES entries(id) ON DELETE CASCADE,
    ADD CONSTRAINT entry_relations_b_id_fkey
        FOREIGN KEY (b_id) REFERENCES entries(id) ON DELETE CASCADE;

ALTER TABLE entry_types
    ADD CONSTRAINT entry_types_project_id_fkey
        FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE;

CREATE OR REPLACE FUNCTION normalize_relation_order()
    RETURNS TRIGGER AS $$
DECLARE
    tmp UUID;
BEGIN
    IF NEW.relation = 'two_way' AND NEW.a_id > NEW.b_id THEN
        tmp := NEW.a_id;
        NEW.a_id := NEW.b_id;
        NEW.b_id := tmp;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
