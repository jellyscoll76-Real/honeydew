-- Private, unlisted BanaForge project-share objects. Access is through the project-share Edge Function only.
create extension if not exists pgcrypto;

insert into storage.buckets (id, name, public, file_size_limit, allowed_mime_types)
values (
  'project-shares',
  'project-shares',
  false,
  134217728,
  array['application/zip', 'application/octet-stream']
)
on conflict (id) do update set
  public = false,
  file_size_limit = excluded.file_size_limit,
  allowed_mime_types = excluded.allowed_mime_types;

create table if not exists public.project_shares (
  share_hash text primary key check (share_hash ~ '^[0-9a-f]{64}$'),
  owner_hash text not null check (owner_hash ~ '^[0-9a-f]{64}$'),
  object_path text not null unique,
  project_name text not null check (char_length(project_name) between 1 and 100),
  expected_size bigint not null check (expected_size between 1 and 134217728),
  uploaded boolean not null default false,
  created_at timestamptz not null default now(),
  expires_at timestamptz not null,
  check (expires_at > created_at)
);

alter table public.project_shares enable row level security;
revoke all on table public.project_shares from anon, authenticated;
grant all on table public.project_shares to service_role;

-- Per-IP hashes are salted by an Edge Function secret; raw IPs are never stored.
create table if not exists public.project_share_rate_limits (
  ip_hash text primary key check (ip_hash ~ '^[0-9a-f]{64}$'),
  window_started_at timestamptz not null,
  create_count integer not null check (create_count >= 0)
);
alter table public.project_share_rate_limits enable row level security;
revoke all on table public.project_share_rate_limits from anon, authenticated;
grant all on table public.project_share_rate_limits to service_role;

create or replace function public.consume_project_share_quota(p_ip_hash text)
returns boolean
language plpgsql
security definer
set search_path = public, pg_temp
as $$
declare
  new_count integer;
begin
  insert into public.project_share_rate_limits (ip_hash, window_started_at, create_count)
  values (p_ip_hash, now(), 1)
  on conflict (ip_hash) do update set
    window_started_at = case
      when public.project_share_rate_limits.window_started_at < now() - interval '1 hour' then now()
      else public.project_share_rate_limits.window_started_at
    end,
    create_count = case
      when public.project_share_rate_limits.window_started_at < now() - interval '1 hour' then 1
      else public.project_share_rate_limits.create_count + 1
    end
  returning create_count into new_count;
  return new_count <= 5;
end;
$$;
revoke all on function public.consume_project_share_quota(text) from public, anon, authenticated;
grant execute on function public.consume_project_share_quota(text) to service_role;

create index if not exists project_shares_expiry_idx on public.project_shares (expires_at);
