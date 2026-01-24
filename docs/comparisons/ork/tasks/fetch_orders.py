import time
from pydantic import BaseModel


class FetchOrdersInput(BaseModel):
    pass


class FetchOrdersOutput(BaseModel):
    status: str


def main(input: FetchOrdersInput) -> FetchOrdersOutput:
    time.sleep(1)
    print("Fetched orders")
    return FetchOrdersOutput(status="done")
