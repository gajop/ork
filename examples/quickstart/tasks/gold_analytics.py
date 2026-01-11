from pydantic import BaseModel

from src.pipeline import create_gold_analytics


class SilverOutput(BaseModel):
    source: str
    bronze_count: int
    silver_count: int


class GoldAnalyticsInput(BaseModel):
    silver_posts: SilverOutput
    silver_comments: SilverOutput
    silver_users: SilverOutput
    db_path: str


class GoldAnalyticsOutput(BaseModel):
    total_users: int
    total_posts: int
    total_comments: int
    avg_comments_per_user: float


def main(input: GoldAnalyticsInput) -> GoldAnalyticsOutput:
    stats = create_gold_analytics(input.db_path)
    return GoldAnalyticsOutput(**stats)
