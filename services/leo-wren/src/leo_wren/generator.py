from __future__ import annotations

import re
import shutil
from pathlib import Path
from typing import Any

import yaml


class WrenGenerator:
    """Generate a Wren project from a leo-pgjson schema dump."""

    SENSITIVE_TABLES = {"database_connections", "secrets"}
    SENSITIVE_COLUMNS = {
        "api_key",
        "password",
        "password_ciphertext",
        "password_nonce",
        "tool_call_id",
        "tool_calls",
    }

    def __init__(
        self,
        dump: dict[str, Any],
        *,
        name: str | None = None,
        profile: str = "leo-local",
        include_sensitive: bool = False,
    ) -> None:
        self.dump = dump
        self.name = name or f"{dump.get('database', 'database')}_analytics"
        self.profile = profile
        self.include_sensitive = include_sensitive

    def generate(self, output: Path) -> None:
        dumped_tables = self.dump.get("tables")
        if not isinstance(dumped_tables, list):
            raise ValueError("el JSON debe contener un arreglo 'tables'")
        tables = [
            table
            for table in dumped_tables
            if self.include_sensitive or table.get("name") not in self.SENSITIVE_TABLES
        ]

        output.mkdir(parents=True, exist_ok=True)
        models_dir = output / "models"
        if models_dir.exists():
            shutil.rmtree(models_dir)
        models_dir.mkdir()

        model_names = self._model_names(tables)
        relationships: list[dict[str, Any]] = []
        relation_columns: dict[str, list[dict[str, Any]]] = {
            model: [] for model in model_names.values()
        }
        for table in tables:
            source_key = self._table_key(table)
            source_model = model_names[source_key]
            for foreign_key in table.get("foreign_keys", []):
                if not self.include_sensitive and any(
                    column in self.SENSITIVE_COLUMNS
                    for column in foreign_key.get("columns", [])
                ):
                    continue
                target_key = (
                    foreign_key["referenced_schema"],
                    foreign_key["referenced_table"],
                )
                target_model = model_names.get(target_key)
                if target_model is None:
                    continue
                relationship = self._relationship(
                    foreign_key, source_model, target_model
                )
                relationships.append(relationship)
                relationship_name = relationship["name"]
                source_relation = self._identifier(
                    str(foreign_key["columns"][0]).removesuffix("_id")
                    if len(foreign_key["columns"]) == 1
                    else target_model
                )
                relation_columns[source_model].append(
                    {
                        "name": self._unique_relation_name(
                            source_relation,
                            table.get("columns", []),
                            relation_columns[source_model],
                        ),
                        "type": target_model,
                        "relationship": relationship_name,
                    }
                )
                relation_columns[target_model].append(
                    {
                        "name": self._unique_relation_name(
                            f"{source_model}_{source_relation}",
                            self._table_for(target_key, tables).get("columns", []),
                            relation_columns[target_model],
                        ),
                        "type": source_model,
                        "relationship": relationship_name,
                    }
                )

        default_schema = self._default_schema(tables)
        self._write_yaml(
            output / "wren_project.yml",
            {
                "schema_version": 5,
                "name": self.name,
                "version": "0.1",
                "catalog": "wren",
                "schema": default_schema,
                "data_source": "postgres",
                "profile": self.profile,
            },
        )
        self._write_yaml(output / "relationships.yml", {"relationships": relationships})

        for table in tables:
            model_name = model_names[self._table_key(table)]
            metadata = self._metadata(table, model_name)
            metadata["columns"].extend(relation_columns[model_name])
            model_dir = models_dir / model_name
            model_dir.mkdir()
            self._write_yaml(model_dir / "metadata.yml", metadata)

    @staticmethod
    def _table_key(table: dict[str, Any]) -> tuple[str, str]:
        return str(table.get("schema", "public")), str(table["name"])

    def _model_names(self, tables: list[dict[str, Any]]) -> dict[tuple[str, str], str]:
        counts: dict[str, int] = {}
        for table in tables:
            name = self._identifier(str(table["name"]))
            counts[name] = counts.get(name, 0) + 1
        return {
            self._table_key(table): (
                self._identifier(str(table["name"]))
                if counts[self._identifier(str(table["name"]))] == 1
                else self._identifier(f"{table.get('schema', 'public')}_{table['name']}")
            )
            for table in tables
        }

    def _metadata(self, table: dict[str, Any], model_name: str) -> dict[str, Any]:
        columns = []
        primary_keys = []
        for column in table.get("columns", []):
            if not self.include_sensitive and column.get("name") in self.SENSITIVE_COLUMNS:
                continue
            item = {
                "name": column["name"],
                "type": self._wren_type(str(column["data_type"])),
            }
            if column.get("is_primary_key"):
                item["is_primary_key"] = True
                primary_keys.append(column["name"])
            if column.get("not_null"):
                item["not_null"] = True
            columns.append(item)
        metadata: dict[str, Any] = {
            "name": model_name,
            "table_reference": {
                "schema": table.get("schema", "public"),
                "table": table["name"],
            },
        }
        if len(primary_keys) == 1:
            metadata["primary_key"] = primary_keys[0]
        metadata["columns"] = columns
        return metadata

    def _relationship(
        self,
        foreign_key: dict[str, Any],
        source_model: str,
        target_model: str,
    ) -> dict[str, Any]:
        pairs = zip(foreign_key["referenced_columns"], foreign_key["columns"])
        condition = " AND ".join(
            f"{target_model}.{target} = {source_model}.{source}" for target, source in pairs
        )
        return {
            "name": self._identifier(str(foreign_key["name"])),
            "models": [target_model, source_model],
            "join_type": "ONE_TO_MANY",
            "condition": condition,
        }

    @staticmethod
    def _table_for(
        key: tuple[str, str], tables: list[dict[str, Any]]
    ) -> dict[str, Any]:
        return next(
            table
            for table in tables
            if (table.get("schema", "public"), table["name"]) == key
        )

    @staticmethod
    def _default_schema(tables: list[dict[str, Any]]) -> str:
        schemas = {str(table.get("schema", "public")) for table in tables}
        return next(iter(schemas)) if len(schemas) == 1 else "public"

    @staticmethod
    def _identifier(value: str) -> str:
        identifier = re.sub(r"[^A-Za-z0-9_]", "_", value)
        return identifier if identifier and not identifier[0].isdigit() else f"m_{identifier}"

    @staticmethod
    def _unique_relation_name(
        name: str,
        columns: list[dict[str, Any]],
        relationships: list[dict[str, Any]],
    ) -> str:
        used = {str(column.get("name")) for column in columns}
        used.update(str(column["name"]) for column in relationships)
        candidate = name
        suffix = 2
        while candidate in used:
            candidate = f"{name}_{suffix}"
            suffix += 1
        return candidate

    @staticmethod
    def _wren_type(pg_type: str) -> str:
        normalized = pg_type.lower()
        if normalized.endswith("[]"):
            return "JSON"
        if normalized in {"smallint", "integer", "bigint", "smallserial", "serial", "bigserial"}:
            return "BIGINT"
        if normalized.startswith(("numeric", "decimal", "real", "double precision")):
            return "DOUBLE"
        if normalized == "boolean":
            return "BOOLEAN"
        if normalized == "date":
            return "DATE"
        if normalized.startswith("timestamp with time zone") or normalized == "timestamptz":
            return "TIMESTAMPTZ"
        if normalized.startswith("timestamp"):
            return "TIMESTAMP"
        if normalized.startswith("time"):
            return "TIME"
        if normalized in {"json", "jsonb"}:
            return "JSON"
        return "VARCHAR"

    @staticmethod
    def _write_yaml(path: Path, value: dict[str, Any]) -> None:
        path.write_text(
            yaml.safe_dump(value, sort_keys=False, allow_unicode=True),
            encoding="utf-8",
        )
