import json

import yaml

from leo_wren import cli
from leo_wren.generator import WrenGenerator


DUMP = {
    "database": "leo",
    "tables": [
        {
            "schema": "public",
            "name": "parents",
            "columns": [
                {
                    "name": "id",
                    "data_type": "uuid",
                    "not_null": True,
                    "is_primary_key": True,
                }
            ],
            "foreign_keys": [],
            "rows": [],
        },
        {
            "schema": "public",
            "name": "children",
            "columns": [
                {
                    "name": "id",
                    "data_type": "bigint",
                    "not_null": True,
                    "is_primary_key": True,
                },
                {
                    "name": "parent_id",
                    "data_type": "uuid",
                    "not_null": True,
                    "is_primary_key": False,
                },
                {
                    "name": "api_key",
                    "data_type": "text",
                    "not_null": False,
                    "is_primary_key": False,
                },
            ],
            "foreign_keys": [
                {
                    "name": "children_parent_id_fkey",
                    "columns": ["parent_id"],
                    "referenced_schema": "public",
                    "referenced_table": "parents",
                    "referenced_columns": ["id"],
                }
            ],
            "rows": [],
        },
    ],
}


def load(path):
    return yaml.safe_load(path.read_text(encoding="utf-8"))


def test_generator_writes_complete_wren_project(tmp_path):
    WrenGenerator(DUMP).generate(tmp_path)

    project = load(tmp_path / "wren_project.yml")
    child = load(tmp_path / "models" / "children" / "metadata.yml")
    relationships = load(tmp_path / "relationships.yml")["relationships"]

    assert project["name"] == "leo_analytics"
    assert child["primary_key"] == "id"
    assert child["columns"][0]["type"] == "BIGINT"
    assert all(column["name"] != "api_key" for column in child["columns"])
    assert child["columns"][-1]["relationship"] == "children_parent_id_fkey"
    assert relationships[0]["condition"] == "parents.id = children.parent_id"


def test_generate_command_reads_rust_json(tmp_path, capsys):
    source = tmp_path / "dump.json"
    output = tmp_path / "wren"
    source.write_text(json.dumps(DUMP), encoding="utf-8")

    assert cli.main(["generate", str(source), "--output", str(output)]) == 0
    assert (output / "models" / "parents" / "metadata.yml").is_file()
    assert "Wren project generated" in capsys.readouterr().out
