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

class Publisher
  class << self
    def publish; end

    def run
      self.publish
      Publisher.publish
    end
  end
end

class BlockOwner
  self.direct
  [1].each { self.inherited }
  self.class_eval { self.current_class_eval }
  BlockOwner.instance_eval { self.current_instance_eval }
  CALLBACK = proc { self.expression_inherited }

  target.instance_eval { self.foreign_instance_eval }
  Other.class_eval { self.foreign_class_eval }
  RESULT = Other.class_eval { self.foreign_expression_eval }
  Class.new { self.anonymous_class }
  concern :Nested do
    self.nested_concern
  end
end

module ConcernOwner
  extend ActiveSupport::Concern
  self.direct_concern

  included do
    self.included_hook
  end

  class_methods do
    self.class_methods_hook
  end
end
