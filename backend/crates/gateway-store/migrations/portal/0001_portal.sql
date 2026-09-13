-- Portal 用户、套餐、订阅与用户账本。用户只禁用不物理删除。

create table portal_users (
    id text primary key,
    username text not null,
    password_hash text not null,
    status text not null,
    created_at timestamptz not null,
    updated_at timestamptz not null,
    constraint portal_users_username_ck check (
        octet_length(username) between 1 and 64
        and username = btrim(username)
        and username !~ '[[:cntrl:]]'
    ),
    constraint portal_users_status_ck check (status in ('active', 'disabled')),
    constraint portal_users_time_ck check (created_at <= updated_at)
);
create unique index portal_users_username_uq on portal_users (lower(username));

create table portal_sessions (
    id text primary key,
    user_id text not null references portal_users (id) on update restrict on delete restrict,
    token_hash text not null unique,
    expires_at timestamptz not null,
    created_at timestamptz not null
);
create index portal_sessions_user_idx on portal_sessions (user_id, expires_at desc);

create table subscription_plans (
    id text primary key,
    name text not null,
    daily_limit_usd numeric(20, 10) not null default 0 check (daily_limit_usd >= 0),
    weekly_limit_usd numeric(20, 10) not null default 0 check (weekly_limit_usd >= 0),
    max_concurrency bigint not null default 0 check (max_concurrency >= 0),
    requests_per_minute bigint not null default 0 check (requests_per_minute >= 0),
    max_keys bigint not null default 0 check (max_keys >= 0),
    enabled boolean not null default true,
    created_at timestamptz not null,
    updated_at timestamptz not null,
    constraint subscription_plans_name_ck check (
        octet_length(name) between 1 and 64
        and name = btrim(name)
        and name !~ '[[:cntrl:]]'
    ),
    constraint subscription_plans_time_ck check (created_at <= updated_at)
);

create table plan_account_groups (
    plan_id text not null references subscription_plans (id) on update restrict on delete cascade,
    account_group_id text not null references account_groups (id) on update restrict on delete restrict,
    created_at timestamptz not null,
    primary key (plan_id, account_group_id)
);

create table user_subscriptions (
    id text primary key,
    user_id text not null references portal_users (id) on update restrict on delete restrict,
    plan_id text not null references subscription_plans (id) on update restrict on delete restrict,
    status text not null,
    starts_at timestamptz not null,
    ends_at timestamptz,
    created_at timestamptz not null,
    updated_at timestamptz not null,
    constraint user_subscriptions_status_ck check (status in ('active', 'disabled')),
    constraint user_subscriptions_window_ck check (ends_at is null or ends_at > starts_at),
    constraint user_subscriptions_time_ck check (created_at <= updated_at)
);
create unique index user_subscriptions_one_active
    on user_subscriptions (user_id)
    where status = 'active';
create index user_subscriptions_user_idx
    on user_subscriptions (user_id, created_at desc);

create table portal_user_budget_windows (
    user_id text primary key references portal_users (id) on update restrict on delete restrict,
    daily_start timestamptz not null,
    daily_end timestamptz not null,
    weekly_start timestamptz not null,
    weekly_end timestamptz not null,
    daily_used_usd numeric(20, 10) not null default 0 check (daily_used_usd >= 0),
    weekly_used_usd numeric(20, 10) not null default 0 check (weekly_used_usd >= 0)
);

create table portal_user_charge_events (
    request_id text primary key,
    user_id text not null references portal_users (id) on update restrict on delete restrict,
    amount_usd numeric(20, 10) not null check (amount_usd >= 0),
    completed_at timestamptz not null
);
create index portal_user_charge_events_user_idx
    on portal_user_charge_events (user_id, completed_at desc);

create table portal_audit_events (
    id text primary key,
    actor_kind text not null,
    actor_user_id text references portal_users (id) on update restrict on delete set null,
    actor_ref text not null,
    request_id text,
    action text not null,
    entity_kind text not null,
    entity_ref text not null,
    config_revision bigint,
    changed_fields text[] not null default '{}',
    created_at timestamptz not null,
    constraint portal_audit_events_actor_kind_ck check (
        actor_kind in ('portal_session', 'admin_session', 'system')
    ),
    constraint portal_audit_events_changed_fields_ck check (
        cardinality(changed_fields) <= 64
        and array_position(changed_fields, null) is null
    )
);
create index portal_audit_events_created_idx
    on portal_audit_events (created_at desc, id desc);

alter table client_api_keys
    add column owner_user_id text references portal_users (id) on update restrict on delete restrict;
create index client_api_keys_owner_idx
    on client_api_keys (owner_user_id)
    where owner_user_id is not null;

alter table model_requests
    add column owner_user_id text references portal_users (id) on update restrict on delete restrict;
create index model_requests_owner_idx
    on model_requests (owner_user_id, started_at desc, id desc)
    where owner_user_id is not null;

alter table model_requests drop constraint model_requests_routing_scope_ck;
alter table model_requests add constraint model_requests_routing_scope_ck check (
    routing_scope in ('legacy_provider', 'all', 'groups', 'empty')
);

alter table model_requests drop constraint model_requests_routing_group_names_ck;
alter table model_requests add constraint model_requests_routing_group_names_ck check (
    jsonb_typeof(routing_group_names_snapshot) = 'array'
    and (
        (
            routing_scope in ('legacy_provider', 'all', 'empty')
            and cardinality(routing_group_refs) = 0
            and jsonb_array_length(routing_group_names_snapshot) = 0
        )
        or (
            routing_scope = 'groups'
            and cardinality(routing_group_refs) > 0
            and array_position(routing_group_refs, null) is null
            and jsonb_array_length(routing_group_names_snapshot)
              = cardinality(routing_group_refs)
        )
    )
);
