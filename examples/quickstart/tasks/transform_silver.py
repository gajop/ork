from pydantic import BaseModel

from src.pipeline import transform_to_silver


class BronzeOutput(BaseModel):
    source: str
    count: int


class TransformSilverInput(BaseModel):
    bronze: BronzeOutput
    db_path: str


class TransformSilverOutput(BaseModel):
    source: str
    bronze_count: int
    silver_count: int


def main(input: TransformSilverInput) -> TransformSilverOutput:
    silver_count = transform_to_silver(input.bronze.source, input.db_path)
    return TransformSilverOutput(
        source=input.bronze.source,
        bronze_count=input.bronze.count,
        silver_count=silver_count,
    )
