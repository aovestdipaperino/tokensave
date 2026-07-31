class ReceiverExamples
  def run(worker, account, user)
    save()
    self.save
    Account.find
    Services::Capture.call
    worker.perform
    worker::perform
    worker.(self)
    @client.call
    @@registry.fetch
    account.owner.notify
    user&.profile
    "text".strip
    Array.new
  end
end
