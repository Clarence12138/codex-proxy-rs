-- 位置配置由代理持有；现有代理保持全局继承，不回填账号或凭据。
alter table outbound_proxies
    add column location_country text,
    add column location_region text,
    add column location_city text,
    add column location_timezone text,
    add constraint outbound_proxies_location_complete check (
        num_nonnulls(location_country, location_region, location_city, location_timezone) = 0
        or (
            num_nonnulls(location_country, location_region, location_city, location_timezone) = 4
            and location_country ~ '^[A-Z]{2}$'
            and char_length(btrim(location_region)) between 1 and 128
            and char_length(btrim(location_city)) between 1 and 128
            and char_length(location_timezone) > 0
        )
    );
