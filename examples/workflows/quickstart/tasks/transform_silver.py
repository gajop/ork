from typing import Optional

from pydantic import BaseModel

from src.pipeline import transform_to_silver


class BronzeOutput(BaseModel):
    source: str
    count: int


class TransformSilverInput(BaseModel):
    source: Optional[str] = None
    db_path: str
    bronze: Optional[BronzeOutput] = None
    upstream: Optional[dict[str, BronzeOutput]] = None


class TransformSilverOutput(BaseModel):
    source: str
    bronze_count: int
    silver_count: int


def resolve_bronze(input: TransformSilverInput) -> BronzeOutput:
    if input.bronze is not None:
        return input.bronze
    if input.upstream:
        return next(iter(input.upstream.values()))
    if input.source is not None:
        return BronzeOutput(source=input.source, count=0)
    raise ValueError("transform_silver requires bronze, upstream, or source")


def main(input: TransformSilverInput) -> TransformSilverOutput:
    bronze = resolve_bronze(input)
    silver_count = transform_to_silver(bronze.source, input.db_path)
    return TransformSilverOutput(
        source=bronze.source,
        bronze_count=bronze.count,
        silver_count=silver_count,
    )
