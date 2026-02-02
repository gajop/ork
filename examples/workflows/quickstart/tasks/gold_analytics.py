from typing import Optional

from pydantic import BaseModel

from src.pipeline import create_gold_analytics


class SilverOutput(BaseModel):
    source: str
    bronze_count: int
    silver_count: int


class GoldAnalyticsInput(BaseModel):
    db_path: str
    silver_posts: Optional[SilverOutput] = None
    silver_comments: Optional[SilverOutput] = None
    silver_users: Optional[SilverOutput] = None
    upstream: Optional[dict[str, SilverOutput]] = None


class GoldAnalyticsOutput(BaseModel):
    total_users: int
    total_posts: int
    total_comments: int
    avg_comments_per_user: float


def main(input: GoldAnalyticsInput) -> GoldAnalyticsOutput:
    stats = create_gold_analytics(input.db_path)
    return GoldAnalyticsOutput(**stats)
