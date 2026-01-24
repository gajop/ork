import time
from pydantic import BaseModel


class FetchUsersOutput(BaseModel):
    status: str


class FetchOrdersOutput(BaseModel):
    status: str


class FetchProductsOutput(BaseModel):
    status: str


class CombineDataInput(BaseModel):
    fetch_users: FetchUsersOutput
    fetch_orders: FetchOrdersOutput
    fetch_products: FetchProductsOutput


class CombineDataOutput(BaseModel):
    status: str


def main(input: CombineDataInput) -> CombineDataOutput:
    time.sleep(1)
    print("Combined all data")
    return CombineDataOutput(status="done")
