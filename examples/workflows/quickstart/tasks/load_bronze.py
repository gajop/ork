from pydantic import BaseModel

from src.pipeline import load_to_bronze


class LoadBronzeInput(BaseModel):
    source: str
    date: str
    db_path: str


class LoadBronzeOutput(BaseModel):
    source: str
    count: int


def main(input: LoadBronzeInput) -> LoadBronzeOutput:
    count = load_to_bronze(input.source, input.date, input.db_path)
    return LoadBronzeOutput(source=input.source, count=count)
