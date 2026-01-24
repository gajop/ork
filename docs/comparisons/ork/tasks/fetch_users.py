import time
from pydantic import BaseModel


class FetchUsersInput(BaseModel):
    pass


class FetchUsersOutput(BaseModel):
    status: str


def main(input: FetchUsersInput) -> FetchUsersOutput:
    time.sleep(1)
    print("Fetched users")
    return FetchUsersOutput(status="done")
