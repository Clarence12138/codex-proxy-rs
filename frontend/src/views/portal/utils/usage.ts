export function outcomeText(outcome: string) {
  const labels: Record<string, string> = {
    succeeded: '成功',
    failed: '失败',
    cancelled: '已取消',
    incomplete: '未完成',
    running: '进行中',
  }
  return labels[outcome] ?? '未知状态'
}

export function outcomeClass(outcome: string) {
  if (outcome === 'succeeded')
    return 'bg-cp-success-container text-cp-success-on-container'
  if (outcome === 'failed')
    return 'bg-cp-error-container text-cp-error-on-container'
  if (outcome === 'running')
    return 'bg-cp-info-container text-cp-info-on-container'
  return 'bg-cp-warning-container text-cp-warning-on-container'
}
