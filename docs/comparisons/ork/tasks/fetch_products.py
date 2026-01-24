import time
from pydantic import BaseModel


class FetchProductsInput(BaseModel):
    pass


class FetchProductsOutput(BaseModel):
    status: str


def main(input: FetchProductsInput) -> FetchProductsOutput:
    time.sleep(1)
    print("Fetched products")
    return FetchProductsOutput(status="done")
