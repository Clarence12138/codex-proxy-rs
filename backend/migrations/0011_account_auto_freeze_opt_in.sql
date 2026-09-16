-- 自动冻结默认关闭；保留已经通过设置页显式保存的冻结配置。
alter table runtime_settings
    alter column account_auto_freeze_enabled set default false;

update runtime_settings
set account_auto_freeze_enabled = false
where account_auto_freeze_enabled = true
  and not exists (
    select 1 from admin_audit_events
    where action = 'settings.replace'
      and changed_fields @> array['account_auto_freeze']::text[]
  );
