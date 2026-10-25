module TaskHelpers
  def self.prepare
    puts "Preparing"
  end
end

namespace :maintenance do
  task :prepare do
    TaskHelpers.prepare
  end
end

class TaskRunner
  include SharedHelpers

  def run
    TaskHelpers.prepare
  end
end
