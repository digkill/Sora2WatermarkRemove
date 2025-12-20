-- Map internal products to lava.top offerId

ALTER TABLE products
    ADD COLUMN lava_offer_id UUID;

CREATE INDEX IF NOT EXISTS idx_products_lava_offer_id
    ON products(lava_offer_id);
